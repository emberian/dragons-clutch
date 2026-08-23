//! Canonical hostile-byte decoding and fork-aware untrusted account indexing.
//!
//! The index retains exact bytes and provenance. Its projections help an
//! operator choose work, but never replace onchain account authentication.

use crate::rpc_index::{
    CanonicalFamily, IndexedProgramRelease, ObservedRpcAccount, ObservedSlot, ObservedSlotUpdate,
    ObservedSlotUpdateKind, RpcCommitment, RpcIndexPlan,
};
use crate::workflow_graph::{WorkflowLane, WorkflowPosition};
use clutch_dealer_runtime_contract::{
    DealerFacilityReplayV1, DealerLeaseV1, DealerPolicyV1, DealerStateV1, FeeBudgetV1,
    FixedCodec as DealerFixedCodec, LivenessBudgetV1, LpPageV1, SettlementPotV1,
    DEALER_BUDGET_BYTES_V1, DEALER_FACILITY_REPLAY_BYTES_V1, DEALER_LEASE_BYTES_V1,
    DEALER_LEASE_MAGIC_V1, DEALER_POLICY_BYTES_V1, DEALER_POLICY_MAGIC_V1, DEALER_STATE_BYTES_V1,
    DEALER_STATE_MAGIC_V1, FEE_BUDGET_MAGIC_V1, LIVENESS_BUDGET_MAGIC_V1, LP_PAGE_BYTES_V1,
    LP_PAGE_MAGIC_V1, SETTLEMENT_POT_BYTES_V1, SETTLEMENT_POT_MAGIC_V1,
};
use clutch_general_v2_contract::{
    complete_candidate_feed_v2, AdmissionNodeStatusV1, AdmissionNodeV3AccountV1,
    CandidateWindowV4AccountV1, ClearWorkHeaderV2, EconomicDomainV2AccountV1,
    EpochBudgetV2AccountV1, GeneralEpochV6AccountV1, MarketBindingV1, MarketRuntimeV3AccountV1,
    OwnerSettlementV1AccountV1, SelectedCandidateV1AccountV1, SettlementCashPotV1AccountV1,
    ADMISSION_NODE_ACCOUNT_TAG, ADMISSION_NODE_ACCOUNT_VERSION, CANDIDATE_FEED_ACCOUNT_TAG,
    CANDIDATE_FEED_ACCOUNT_VERSION, CANDIDATE_FEED_STAGE_ACCOUNT_TAG,
    CANDIDATE_FEED_STAGE_ACCOUNT_VERSION, CLEAR_WORK_ACCOUNT_TAG, CLEAR_WORK_ACCOUNT_VERSION,
    ECONOMIC_DOMAIN_ACCOUNT_TAG, ECONOMIC_DOMAIN_ACCOUNT_VERSION, EPOCH_BUDGET_ACCOUNT_TAG,
    EPOCH_BUDGET_ACCOUNT_VERSION, FINAL_POT_ACCOUNT_BYTES, FINAL_POT_ACCOUNT_TAG,
    FINAL_POT_ACCOUNT_VERSION, GENERAL_EPOCH_ACCOUNT_TAG, GENERAL_EPOCH_ACCOUNT_VERSION,
    MARKET_BINDING_ACCOUNT_TAG, MARKET_BINDING_ACCOUNT_VERSION, MARKET_RUNTIME_ACCOUNT_TAG,
    MARKET_RUNTIME_ACCOUNT_VERSION, OWNER_FEE_CARRY_ACCOUNT_BYTES, OWNER_FEE_CARRY_ACCOUNT_TAG,
    OWNER_FEE_CARRY_ACCOUNT_VERSION, OWNER_SETTLEMENT_ACCOUNT_TAG,
    OWNER_SETTLEMENT_ACCOUNT_VERSION, PAYER_ALLOCATION_ACCOUNT_BYTES, PAYER_ALLOCATION_ACCOUNT_TAG,
    PAYER_ALLOCATION_ACCOUNT_VERSION, RECIPIENT_ALLOCATION_ACCOUNT_BYTES,
    RECIPIENT_ALLOCATION_ACCOUNT_TAG, RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
    SELECTED_CANDIDATE_ACCOUNT_TAG, SELECTED_CANDIDATE_ACCOUNT_VERSION,
    SELECTED_FEE_RECORD_ACCOUNT_BYTES, SELECTED_FEE_RECORD_ACCOUNT_TAG,
    SELECTED_FEE_RECORD_ACCOUNT_VERSION, SETTLEMENT_CASH_POT_ACCOUNT_TAG,
    SETTLEMENT_CASH_POT_ACCOUNT_VERSION, TREASURY_LEDGER_ACCOUNT_BYTES,
    TREASURY_LEDGER_ACCOUNT_TAG, TREASURY_LEDGER_ACCOUNT_VERSION, WINDOW_ACCOUNT_TAG,
    WINDOW_ACCOUNT_VERSION,
};
use clutch_liveness::{
    RuntimeCompartmentPhaseV1, RuntimeCompartmentV1, RuntimeLivenessPolicyV1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_ACCOUNT_MAGIC_V1,
    RUNTIME_LIVENESS_POLICY_BYTES_V1, RUNTIME_LIVENESS_POLICY_MAGIC_V1,
};
use clutch_retirement::{
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, ReplayV3Envelope,
    ReplayV3HashBackend, ReplayV3Lifecycle, POSITION_ACCOUNT_TAG, POSITION_ACCOUNT_VERSION_V3,
    PURPOSE_REPLAY_ACCOUNT_TAG, PURPOSE_REPLAY_ACCOUNT_VERSION_V3,
};
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FailureReplayTombstoneV1,
    FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1, FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1, FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2,
    FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1, FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
};
use clutch_solana_layout::product_series::{SeriesFundingAccountV1, SeriesRegistryAccountV1};
use clutch_solana_layout::registry;
use clutch_source_plane_v3::{
    OpenRawPageV3, RawPageV3, SourceHeadV3, StatisticResultV3, WindowSealV3, WindowWorkV3,
    MAX_RAW_PAGE_RECORDS,
};
use clutch_source_plane_v3_runtime::{
    decode_runtime_account, ReopenLineageV1, RuntimeKey, SourceReleaseManifestV1,
    SourceWorkReceiptAccountV1, OPEN_RAW_PAGE_ACCOUNT_TAG, RAW_PAGE_ACCOUNT_TAG,
    REOPEN_LINEAGE_ACCOUNT_TAG, REOPEN_LINEAGE_ACCOUNT_VERSION, SOURCE_HEAD_ACCOUNT_TAG,
    SOURCE_RELEASE_ACCOUNT_TAG, SOURCE_RELEASE_ACCOUNT_VERSION, SOURCE_WORK_RECEIPT_ACCOUNT_BYTES,
    SOURCE_WORK_RECEIPT_ACCOUNT_TAG, SOURCE_WORK_RECEIPT_ACCOUNT_VERSION,
    STATISTIC_RESULT_ACCOUNT_TAG, WINDOW_SEAL_ACCOUNT_TAG, WINDOW_WORK_ACCOUNT_TAG,
};
use clutch_structured_claim_runtime_contract::{
    DescriptorStateV1, StructuredClaimDescriptorV1, DESCRIPTOR_ACCOUNT_BYTES,
    DESCRIPTOR_ACCOUNT_TAG, DESCRIPTOR_ACCOUNT_VERSION,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::{BTreeMap, BTreeSet};

pub type Result<T> = core::result::Result<T, AccountIndexError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountIndexError {
    UnknownRelease,
    WrongCluster,
    WrongOwner,
    ExecutableDataAccount,
    UnknownCodec,
    AmbiguousCodec,
    CanonicalDecodeRefused,
    InvalidFork,
    UnknownFork,
    AmbiguousFork,
    RootRegression,
    StaleObservation,
    CapacityExceeded,
}

impl core::fmt::Display for AccountIndexError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownRelease => "account observation names an unknown release",
            Self::WrongCluster => "account observation names another cluster",
            Self::WrongOwner => "account owner differs from its explicit release",
            Self::ExecutableDataAccount => "executable account cannot enter the data index",
            Self::UnknownCodec => "no canonical account owner recognized the bytes",
            Self::AmbiguousCodec => "multiple canonical owners recognized the same bytes",
            Self::CanonicalDecodeRefused => "canonical account decoder refused the bytes",
            Self::InvalidFork => "slot observation has invalid fork geometry",
            Self::UnknownFork => "account observation cannot be joined to an observed fork",
            Self::AmbiguousFork => "slot maps to more than one observed fork",
            Self::RootRegression => "finalized root would regress",
            Self::StaleObservation => "account observation regresses its receive ordering",
            Self::CapacityExceeded => "account index exceeds its explicit capacity",
        })
    }
}

impl std::error::Error for AccountIndexError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalAccountKind {
    GeneralMarketRuntime,
    GeneralEpoch,
    GeneralEconomicDomain,
    GeneralMarketBinding,
    GeneralCandidateWindow,
    GeneralAdmissionNode,
    GeneralCandidateFeedStage,
    GeneralCandidateFeed,
    GeneralClearWork,
    GeneralSelectedCandidate,
    GeneralEpochBudget,
    GeneralOwnerSettlement,
    GeneralSettlementCashPot,
    GeneralFinalPot,
    SeriesRegistry,
    SeriesFunding,
    SourceRelease,
    SourceHead,
    SourceOpenRawPage,
    SourceRawPage,
    SourceWindowWork,
    SourceWindowSeal,
    SourceStatisticResult,
    SourceLineage,
    SourceWorkReceipt,
    FeeSelectedRecord,
    FeeOwnerCarry,
    FeePayerAllocation,
    FeeRecipientAllocation,
    FeeTreasuryLedger,
    LivenessPolicy,
    LivenessCompartment,
    PositionV3,
    ReplayV3,
    StructuredClaimDescriptor,
    DealerPolicy,
    DealerState,
    DealerLpPage,
    DealerLease,
    DealerSettlementPot,
    DealerFeeBudget,
    DealerLivenessBudget,
    DealerReplay,
    FailureExternalRoot,
    FailureLivenessPolicy,
    FailureRecoveryCompartment,
    FailureReplayTombstone,
}

impl CanonicalAccountKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::GeneralMarketRuntime => "general-market-runtime",
            Self::GeneralEpoch => "general-epoch",
            Self::GeneralEconomicDomain => "general-economic-domain",
            Self::GeneralMarketBinding => "general-market-binding",
            Self::GeneralCandidateWindow => "general-candidate-window",
            Self::GeneralAdmissionNode => "general-admission-node",
            Self::GeneralCandidateFeedStage => "general-candidate-feed-stage",
            Self::GeneralCandidateFeed => "general-candidate-feed",
            Self::GeneralClearWork => "general-clear-work",
            Self::GeneralSelectedCandidate => "general-selected-candidate",
            Self::GeneralEpochBudget => "general-epoch-budget",
            Self::GeneralOwnerSettlement => "general-owner-settlement",
            Self::GeneralSettlementCashPot => "general-settlement-cash-pot",
            Self::GeneralFinalPot => "general-final-pot",
            Self::SeriesRegistry => "series-registry",
            Self::SeriesFunding => "series-funding",
            Self::SourceRelease => "source-release",
            Self::SourceHead => "source-head",
            Self::SourceOpenRawPage => "source-open-raw-page",
            Self::SourceRawPage => "source-raw-page",
            Self::SourceWindowWork => "source-window-work",
            Self::SourceWindowSeal => "source-window-seal",
            Self::SourceStatisticResult => "source-statistic-result",
            Self::SourceLineage => "source-lineage",
            Self::SourceWorkReceipt => "source-work-receipt",
            Self::FeeSelectedRecord => "fee-selected-record",
            Self::FeeOwnerCarry => "fee-owner-carry",
            Self::FeePayerAllocation => "fee-payer-allocation",
            Self::FeeRecipientAllocation => "fee-recipient-allocation",
            Self::FeeTreasuryLedger => "fee-treasury-ledger",
            Self::LivenessPolicy => "liveness-policy",
            Self::LivenessCompartment => "liveness-compartment",
            Self::PositionV3 => "position-v3",
            Self::ReplayV3 => "replay-v3",
            Self::StructuredClaimDescriptor => "structured-claim-descriptor",
            Self::DealerPolicy => "dealer-policy",
            Self::DealerState => "dealer-state",
            Self::DealerLpPage => "dealer-lp-page",
            Self::DealerLease => "dealer-lease",
            Self::DealerSettlementPot => "dealer-settlement-pot",
            Self::DealerFeeBudget => "dealer-fee-budget",
            Self::DealerLivenessBudget => "dealer-liveness-budget",
            Self::DealerReplay => "dealer-replay",
            Self::FailureExternalRoot => "failure-external-root",
            Self::FailureLivenessPolicy => "failure-liveness-policy",
            Self::FailureRecoveryCompartment => "failure-recovery-compartment",
            Self::FailureReplayTombstone => "failure-replay-tombstone",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeState {
    Canonical,
    RequiresContext(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeeperHint {
    pub lane: Option<WorkflowLane>,
    pub position: WorkflowPosition,
    pub action: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalAccountProjection {
    pub family: CanonicalFamily,
    pub kind: CanonicalAccountKind,
    pub decode_state: DecodeState,
    pub generation: Option<u64>,
    pub primary_binding: Option<[u8; 32]>,
    pub secondary_binding: Option<[u8; 32]>,
    pub keeper_hint: Option<KeeperHint>,
}

impl CanonicalAccountProjection {
    fn canonical(family: CanonicalFamily, kind: CanonicalAccountKind) -> Self {
        Self {
            family,
            kind,
            decode_state: DecodeState::Canonical,
            generation: None,
            primary_binding: None,
            secondary_binding: None,
            keeper_hint: None,
        }
    }

    fn contextual(
        family: CanonicalFamily,
        kind: CanonicalAccountKind,
        requirement: &'static str,
    ) -> Self {
        Self {
            decode_state: DecodeState::RequiresContext(requirement),
            ..Self::canonical(family, kind)
        }
    }
}

/// Decoder inputs which are release-wide facts rather than account-local state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalDecoderContext {
    pub source_neutral_sink: RuntimeKey,
}

pub struct CanonicalAccountDecoderRegistry<'a> {
    plan: &'a RpcIndexPlan,
    context: CanonicalDecoderContext,
}

impl<'a> CanonicalAccountDecoderRegistry<'a> {
    #[must_use]
    pub const fn new(plan: &'a RpcIndexPlan, context: CanonicalDecoderContext) -> Self {
        Self { plan, context }
    }

    pub fn decode(&self, account: &ObservedRpcAccount) -> Result<CanonicalAccountProjection> {
        if account.executable {
            return Err(AccountIndexError::ExecutableDataAccount);
        }
        if account.provenance.cluster_key != self.plan.cluster.key() {
            return Err(AccountIndexError::WrongCluster);
        }
        let release = self
            .plan
            .releases
            .iter()
            .find(|release| release.key() == account.provenance.release_key)
            .ok_or(AccountIndexError::UnknownRelease)?;
        if release.program_id != account.owner {
            return Err(AccountIndexError::WrongOwner);
        }
        let mut decoded = None;
        for family in &release.families {
            if let Some(projection) = self.decode_family(*family, account)? {
                if decoded.replace(projection).is_some() {
                    return Err(AccountIndexError::AmbiguousCodec);
                }
            }
        }
        decoded.ok_or(AccountIndexError::UnknownCodec)
    }

    fn decode_family(
        &self,
        family: CanonicalFamily,
        account: &ObservedRpcAccount,
    ) -> Result<Option<CanonicalAccountProjection>> {
        match family {
            CanonicalFamily::General => decode_general(&account.data),
            CanonicalFamily::Source => decode_source(&account.data, self.context),
            CanonicalFamily::Series => decode_series(&account.data),
            CanonicalFamily::Fees => decode_fee(&account.data),
            CanonicalFamily::Liveness => decode_liveness(&account.data),
            CanonicalFamily::PositionV3 => decode_position(&account.data),
            CanonicalFamily::ReplayV3 => decode_replay(&account.data),
            CanonicalFamily::StructuredClaim => decode_structured(&account.data),
            CanonicalFamily::Dealer => decode_dealer(&account.data),
            CanonicalFamily::Failure => decode_failure(&account.data),
        }
    }
}

fn tag_version(data: &[u8], tag: u8, version: u8) -> bool {
    data.first() == Some(&tag) && data.get(1) == Some(&version)
}

fn decode_general(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    let mut projection = if tag_version(
        data,
        MARKET_RUNTIME_ACCOUNT_TAG,
        MARKET_RUNTIME_ACCOUNT_VERSION,
    ) {
        let value = MarketRuntimeV3AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralMarketRuntime,
        );
        projection.generation = Some(value.next_epoch_generation);
        projection.primary_binding = Some(value.market_instance_v2_id.bytes());
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::Creation),
            position: WorkflowPosition {
                phase: 2,
                item: value.next_epoch_index,
            },
            action: "init-epoch",
        });
        projection
    } else if tag_version(
        data,
        GENERAL_EPOCH_ACCOUNT_TAG,
        GENERAL_EPOCH_ACCOUNT_VERSION,
    ) {
        let value = GeneralEpochV6AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralEpoch,
        );
        projection.generation = Some(value.generation);
        projection.primary_binding = Some(value.market_runtime.bytes());
        projection.secondary_binding = Some(value.window.bytes());
        projection
    } else if tag_version(
        data,
        ECONOMIC_DOMAIN_ACCOUNT_TAG,
        ECONOMIC_DOMAIN_ACCOUNT_VERSION,
    ) {
        let value = EconomicDomainV2AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralEconomicDomain,
        );
        projection.primary_binding = Some(value.epoch.bytes());
        projection
    } else if tag_version(
        data,
        MARKET_BINDING_ACCOUNT_TAG,
        MARKET_BINDING_ACCOUNT_VERSION,
    ) {
        let value =
            MarketBindingV1::decode(data).map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralMarketBinding,
        );
        projection.primary_binding = Some(value.market.bytes());
        projection.secondary_binding = Some(value.market_instance_v2_id.bytes());
        projection
    } else if tag_version(data, WINDOW_ACCOUNT_TAG, WINDOW_ACCOUNT_VERSION) {
        let value = CandidateWindowV4AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralCandidateWindow,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection
    } else if tag_version(
        data,
        ADMISSION_NODE_ACCOUNT_TAG,
        ADMISSION_NODE_ACCOUNT_VERSION,
    ) {
        let value = AdmissionNodeV3AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let (lane, phase, action) = match value.status {
            AdmissionNodeStatusV1::Committed => {
                (WorkflowLane::Candidate, 2, "write-candidate-feed")
            }
            AdmissionNodeStatusV1::Revealed => (WorkflowLane::Candidate, 2, "write-candidate-feed"),
            _ => (WorkflowLane::RecoveryRetirement, 2, "cleanup-candidate"),
        };
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralAdmissionNode,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.secondary_binding = Some(value.node.bytes());
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(lane),
            position: WorkflowPosition {
                phase,
                item: value.ordinal,
            },
            action,
        });
        projection
    } else if tag_version(
        data,
        CANDIDATE_FEED_STAGE_ACCOUNT_TAG,
        CANDIDATE_FEED_STAGE_ACCOUNT_VERSION,
    ) {
        let (value, _) = complete_candidate_feed_v2(data, false)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let complete = value.prices_written == value.outcome_count
            && value.fills_written == value.order_count
            && value.atoms_written == value.atom_count
            && value.slices_written == value.slice_count;
        let item = u64::from(value.prices_written)
            + u64::from(value.fills_written)
            + u64::from(value.atoms_written)
            + u64::from(value.slices_written);
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralCandidateFeedStage,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.secondary_binding = Some(value.node.bytes());
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::Candidate),
            position: WorkflowPosition {
                phase: if complete { 4 } else { 3 },
                item,
            },
            action: if complete {
                "seal-candidate"
            } else {
                "write-candidate-feed"
            },
        });
        projection
    } else if tag_version(
        data,
        CANDIDATE_FEED_ACCOUNT_TAG,
        CANDIDATE_FEED_ACCOUNT_VERSION,
    ) {
        let (value, _) = complete_candidate_feed_v2(data, true)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralCandidateFeed,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.secondary_binding = Some(value.node.bytes());
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::Candidate),
            position: WorkflowPosition { phase: 5, item: 0 },
            action: "init-clear-work",
        });
        projection
    } else if tag_version(data, CLEAR_WORK_ACCOUNT_TAG, CLEAR_WORK_ACCOUNT_VERSION) {
        let value = ClearWorkHeaderV2::decode_account(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let (phase, item, action) = match value.phase {
            0 => (6, 0, "grow-clear-work"),
            1 => (7, u64::from(value.order_cursor), "advance-clear-orders"),
            2 => (8, u64::from(value.slice_cursor), "advance-clear-slices"),
            _ => (
                9,
                u64::from(value.slice_cursor),
                "complete-candidate-verification",
            ),
        };
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralClearWork,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.secondary_binding = Some(value.node.bytes());
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::Candidate),
            position: WorkflowPosition { phase, item },
            action,
        });
        projection
    } else if tag_version(
        data,
        SELECTED_CANDIDATE_ACCOUNT_TAG,
        SELECTED_CANDIDATE_ACCOUNT_VERSION,
    ) {
        let value = SelectedCandidateV1AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralSelectedCandidate,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.keeper_hint = (value.entitlement_state < 2).then_some(KeeperHint {
            lane: Some(WorkflowLane::Candidate),
            position: WorkflowPosition {
                phase: 10,
                item: u64::from(value.next_slice_index),
            },
            action: "freeze-entitlement",
        });
        projection
    } else if tag_version(data, EPOCH_BUDGET_ACCOUNT_TAG, EPOCH_BUDGET_ACCOUNT_VERSION) {
        let value = EpochBudgetV2AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralEpochBudget,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection
    } else if tag_version(
        data,
        OWNER_SETTLEMENT_ACCOUNT_TAG,
        OWNER_SETTLEMENT_ACCOUNT_VERSION,
    ) {
        let value = OwnerSettlementV1AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralOwnerSettlement,
        );
        projection.primary_binding = Some(value.semantic.expectation.epoch);
        projection.secondary_binding = Some(value.semantic.expectation.owner);
        // FinalizeOwner's cursor generation is owned by the joined Position V3,
        // not this accumulator or its Epoch. Do not invent a parallel value.
        projection
    } else if tag_version(
        data,
        SETTLEMENT_CASH_POT_ACCOUNT_TAG,
        SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
    ) {
        let value = SettlementCashPotV1AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralSettlementCashPot,
        );
        projection.primary_binding = Some(value.semantic.expectation.epoch);
        projection.secondary_binding = Some(value.semantic.expectation.candidate);
        projection
    } else if tag_version(data, FINAL_POT_ACCOUNT_TAG, FINAL_POT_ACCOUNT_VERSION)
        && data.len() == FINAL_POT_ACCOUNT_BYTES
    {
        CanonicalAccountProjection::contextual(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralFinalPot,
            "authenticated SelectedCandidate and PDA binding",
        )
    } else {
        return Ok(None);
    };
    Ok(Some(projection))
}

fn decode_series(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if tag_version(
        data,
        registry::SOURCE_SERIES_REGISTRY_ACCOUNT_TAG,
        registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION,
    ) {
        let value = SeriesRegistryAccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Series,
            CanonicalAccountKind::SeriesRegistry,
        );
        projection.primary_binding = Some(value.series_plan_id.bytes());
        projection.secondary_binding = Some(value.registry_release_id.bytes());
        Ok(Some(projection))
    } else if tag_version(
        data,
        registry::SOURCE_SERIES_FUNDING_ACCOUNT_TAG,
        registry::SOURCE_SERIES_FUNDING_ACCOUNT_VERSION,
    ) {
        let value = SeriesFundingAccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let terminal = value.state.next_ordinal == value.state.instance_count;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Series,
            CanonicalAccountKind::SeriesFunding,
        );
        projection.generation = Some(1);
        projection.primary_binding = Some(value.state.series_plan_id.bytes());
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(if terminal {
                WorkflowLane::RecoveryRetirement
            } else {
                WorkflowLane::Creation
            }),
            position: WorkflowPosition {
                phase: if terminal { 8 } else { 6 },
                item: u64::from(value.state.next_ordinal),
            },
            action: if terminal {
                "close-series-funding"
            } else {
                "advance-series-occurrence"
            },
        });
        Ok(Some(projection))
    } else {
        Ok(None)
    }
}

fn decode_source(
    data: &[u8],
    context: CanonicalDecoderContext,
) -> Result<Option<CanonicalAccountProjection>> {
    let runtime = |kind, generation, primary| {
        let mut projection = CanonicalAccountProjection::canonical(CanonicalFamily::Source, kind);
        projection.generation = generation;
        projection.primary_binding = primary;
        projection
    };
    let projection = match data.first().copied() {
        Some(SOURCE_RELEASE_ACCOUNT_TAG)
            if data.get(1) == Some(&SOURCE_RELEASE_ACCOUNT_VERSION) =>
        {
            let value = SourceReleaseManifestV1::decode(data)
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceRelease,
                None,
                Some(value.source_spec_id.bytes()),
            );
            projection.secondary_binding = Some(
                value
                    .id()
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?
                    .bytes(),
            );
            projection
        }
        Some(SOURCE_HEAD_ACCOUNT_TAG) => {
            let (header, value) =
                decode_runtime_account::<SourceHeadV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceHead,
                Some(header.generation),
                Some(value.source_spec_id.bytes()),
            );
            projection.keeper_hint = Some(KeeperHint {
                lane: Some(WorkflowLane::SourceCrank),
                position: WorkflowPosition {
                    phase: 2,
                    item: value.page_count,
                },
                action: "open-raw-page",
            });
            projection
        }
        Some(OPEN_RAW_PAGE_ACCOUNT_TAG) => {
            let (header, value) =
                decode_runtime_account::<OpenRawPageV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let full = usize::from(value.record_count) == MAX_RAW_PAGE_RECORDS;
            let next_bucket = value
                .start_bucket
                .checked_add(u64::from(value.record_count))
                .ok_or(AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceOpenRawPage,
                Some(header.generation),
                Some(value.source_spec_id.bytes()),
            );
            projection.keeper_hint = Some(KeeperHint {
                lane: Some(WorkflowLane::SourceCrank),
                position: WorkflowPosition {
                    phase: if full { 4 } else { 3 },
                    item: if full { value.page_index } else { next_bucket },
                },
                action: if full {
                    "seal-raw-page"
                } else {
                    "ingest-boundary"
                },
            });
            projection
        }
        Some(RAW_PAGE_ACCOUNT_TAG) => {
            let (header, _) =
                decode_runtime_account::<RawPageV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            runtime(
                CanonicalAccountKind::SourceRawPage,
                Some(header.generation),
                None,
            )
        }
        Some(WINDOW_WORK_ACCOUNT_TAG) => {
            let (header, _) =
                decode_runtime_account::<WindowWorkV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            runtime(
                CanonicalAccountKind::SourceWindowWork,
                Some(header.generation),
                None,
            )
        }
        Some(WINDOW_SEAL_ACCOUNT_TAG) => {
            let (header, _) =
                decode_runtime_account::<WindowSealV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            runtime(
                CanonicalAccountKind::SourceWindowSeal,
                Some(header.generation),
                None,
            )
        }
        Some(STATISTIC_RESULT_ACCOUNT_TAG) => {
            let (header, _) =
                decode_runtime_account::<StatisticResultV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            runtime(
                CanonicalAccountKind::SourceStatisticResult,
                Some(header.generation),
                None,
            )
        }
        Some(REOPEN_LINEAGE_ACCOUNT_TAG)
            if data.get(1) == Some(&REOPEN_LINEAGE_ACCOUNT_VERSION) =>
        {
            let value = ReopenLineageV1::decode(data)
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceLineage,
                (value.latest_generation > 0).then_some(value.latest_generation),
                Some(value.semantic_binding_id.bytes()),
            );
            projection.secondary_binding = Some(value.active_account.bytes());
            projection
        }
        Some(SOURCE_WORK_RECEIPT_ACCOUNT_TAG)
            if data.get(1) == Some(&SOURCE_WORK_RECEIPT_ACCOUNT_VERSION)
                && data.len() == SOURCE_WORK_RECEIPT_ACCOUNT_BYTES =>
        {
            let value = SourceWorkReceiptAccountV1::decode(data)
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceWorkReceipt,
                Some(value.generation()),
                Some(value.route_id().bytes()),
            );
            projection.secondary_binding = Some(value.receipt_id().bytes());
            projection
        }
        _ => return Ok(None),
    };
    Ok(Some(projection))
}

fn decode_fee(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    let contextual = if tag_version(
        data,
        SELECTED_FEE_RECORD_ACCOUNT_TAG,
        SELECTED_FEE_RECORD_ACCOUNT_VERSION,
    ) && data.len() == SELECTED_FEE_RECORD_ACCOUNT_BYTES
    {
        Some((
            CanonicalAccountKind::FeeSelectedRecord,
            "authenticated batch and revenue-policy preimages",
        ))
    } else if tag_version(
        data,
        OWNER_FEE_CARRY_ACCOUNT_TAG,
        OWNER_FEE_CARRY_ACCOUNT_VERSION,
    ) && data.len() == OWNER_FEE_CARRY_ACCOUNT_BYTES
    {
        Some((
            CanonicalAccountKind::FeeOwnerCarry,
            "authenticated selected fee record",
        ))
    } else if tag_version(
        data,
        PAYER_ALLOCATION_ACCOUNT_TAG,
        PAYER_ALLOCATION_ACCOUNT_VERSION,
    ) && data.len() == PAYER_ALLOCATION_ACCOUNT_BYTES
    {
        Some((
            CanonicalAccountKind::FeePayerAllocation,
            "authenticated fee assessment and signed envelopes",
        ))
    } else if tag_version(
        data,
        RECIPIENT_ALLOCATION_ACCOUNT_TAG,
        RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
    ) && data.len() == RECIPIENT_ALLOCATION_ACCOUNT_BYTES
    {
        Some((
            CanonicalAccountKind::FeeRecipientAllocation,
            "authenticated selected record, revenue policy, and maker rows",
        ))
    } else if tag_version(
        data,
        TREASURY_LEDGER_ACCOUNT_TAG,
        TREASURY_LEDGER_ACCOUNT_VERSION,
    ) && data.len() == TREASURY_LEDGER_ACCOUNT_BYTES
    {
        Some((
            CanonicalAccountKind::FeeTreasuryLedger,
            "authenticated selected fee record",
        ))
    } else {
        None
    };
    Ok(contextual.map(|(kind, requirement)| {
        CanonicalAccountProjection::contextual(CanonicalFamily::Fees, kind, requirement)
    }))
}

fn decode_liveness(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if data.starts_with(&RUNTIME_LIVENESS_POLICY_MAGIC_V1)
        && data.len() == RUNTIME_LIVENESS_POLICY_BYTES_V1
    {
        let _ = RuntimeLivenessPolicyV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        Ok(Some(CanonicalAccountProjection::canonical(
            CanonicalFamily::Liveness,
            CanonicalAccountKind::LivenessPolicy,
        )))
    } else if data.starts_with(&RUNTIME_LIVENESS_ACCOUNT_MAGIC_V1)
        && data.len() == RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
    {
        let value = RuntimeCompartmentV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Liveness,
            CanonicalAccountKind::LivenessCompartment,
        );
        projection.generation = Some(value.identity.generation);
        if value.phase == RuntimeCompartmentPhaseV1::Active && value.remaining_calls > 0 {
            let phase = u16::try_from(value.kind.index())
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            projection.keeper_hint = Some(KeeperHint {
                lane: None,
                position: WorkflowPosition {
                    phase,
                    item: u64::from(value.completed_calls),
                },
                action: "service-liveness-compartment",
            });
        }
        Ok(Some(projection))
    } else {
        Ok(None)
    }
}

fn decode_position(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(data, POSITION_ACCOUNT_TAG, POSITION_ACCOUNT_VERSION_V3) {
        return Ok(None);
    }
    let value =
        PositionAccountV3::decode(data).map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::PositionV3,
        CanonicalAccountKind::PositionV3,
    );
    projection.generation = Some(value.generation());
    projection.primary_binding = Some(value.market_instance_id().bytes());
    projection.secondary_binding = Some(value.replay_account().bytes());
    if value.lifecycle() == PositionLifecycleV3::CloseRequested
        && value.terminal_projection().is_ok()
    {
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::RecoveryRetirement),
            position: WorkflowPosition { phase: 6, item: 0 },
            action: "close-position",
        });
    }
    Ok(Some(projection))
}

struct ReplaySha;

impl ReplayV3HashBackend for ReplaySha {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for part in parts {
            hash.update(part);
        }
        hash.finalize().into()
    }
}

fn decode_replay(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(
        data,
        PURPOSE_REPLAY_ACCOUNT_TAG,
        PURPOSE_REPLAY_ACCOUNT_VERSION_V3,
    ) {
        return Ok(None);
    }
    let value = ReplayV3Envelope::decode(data, &ReplaySha)
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    let header = value.header();
    if header.purpose() == PositionPurposeV3::DealerFacility {
        return Ok(None);
    }
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::ReplayV3,
        CanonicalAccountKind::ReplayV3,
    );
    projection.generation = Some(header.position_generation());
    projection.primary_binding = Some(header.position_account().bytes());
    projection.secondary_binding = Some(header.purpose_binding_id().bytes());
    if header.lifecycle() == ReplayV3Lifecycle::Terminal {
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::RecoveryRetirement),
            position: WorkflowPosition {
                phase: 6,
                item: header.next_sequence(),
            },
            action: "close-position-replay",
        });
    }
    Ok(Some(projection))
}

fn decode_structured(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(data, DESCRIPTOR_ACCOUNT_TAG, DESCRIPTOR_ACCOUNT_VERSION)
        || data.len() != DESCRIPTOR_ACCOUNT_BYTES
    {
        return Ok(None);
    }
    let value = StructuredClaimDescriptorV1::decode(data)
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::StructuredClaim,
        CanonicalAccountKind::StructuredClaimDescriptor,
    );
    projection.primary_binding = Some(value.market);
    if value.state == DescriptorStateV1::Active {
        projection.keeper_hint = Some(KeeperHint {
            lane: None,
            position: WorkflowPosition { phase: 10, item: 0 },
            action: "inspect-structured-claim-retirement",
        });
    }
    Ok(Some(projection))
}

fn decode_dealer(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    let kind = if data.starts_with(&DEALER_POLICY_MAGIC_V1) && data.len() == DEALER_POLICY_BYTES_V1
    {
        let _ = <DealerPolicyV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        CanonicalAccountKind::DealerPolicy
    } else if data.starts_with(&DEALER_STATE_MAGIC_V1) && data.len() == DEALER_STATE_BYTES_V1 {
        let _ = <DealerStateV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        CanonicalAccountKind::DealerState
    } else if data.starts_with(&LP_PAGE_MAGIC_V1) && data.len() == LP_PAGE_BYTES_V1 {
        let _ = <LpPageV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        CanonicalAccountKind::DealerLpPage
    } else if data.starts_with(&DEALER_LEASE_MAGIC_V1) && data.len() == DEALER_LEASE_BYTES_V1 {
        let _ = <DealerLeaseV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        CanonicalAccountKind::DealerLease
    } else if data.starts_with(&SETTLEMENT_POT_MAGIC_V1) && data.len() == SETTLEMENT_POT_BYTES_V1 {
        let _ = <SettlementPotV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        CanonicalAccountKind::DealerSettlementPot
    } else if data.starts_with(&FEE_BUDGET_MAGIC_V1) && data.len() == DEALER_BUDGET_BYTES_V1 {
        let _ = <FeeBudgetV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        CanonicalAccountKind::DealerFeeBudget
    } else if data.starts_with(&LIVENESS_BUDGET_MAGIC_V1) && data.len() == DEALER_BUDGET_BYTES_V1 {
        let _ = <LivenessBudgetV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        CanonicalAccountKind::DealerLivenessBudget
    } else if tag_version(
        data,
        PURPOSE_REPLAY_ACCOUNT_TAG,
        PURPOSE_REPLAY_ACCOUNT_VERSION_V3,
    ) && data.len() == DEALER_FACILITY_REPLAY_BYTES_V1
    {
        let envelope = ReplayV3Envelope::decode(data, &ReplaySha)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        if envelope.header().purpose() != PositionPurposeV3::DealerFacility {
            return Ok(None);
        }
        let value = <DealerFacilityReplayV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerReplay,
        );
        projection.generation = Some(value.position_generation());
        projection.primary_binding = Some(value.facility_position_account_id().bytes());
        projection.secondary_binding = Some(value.facility_position_binding_id().bytes());
        return Ok(Some(projection));
    } else {
        return Ok(None);
    };
    Ok(Some(CanonicalAccountProjection::canonical(
        CanonicalFamily::Dealer,
        kind,
    )))
}

fn decode_failure(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if tag_version(
        data,
        registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION,
    ) && data.len() == FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1
    {
        let _ = decode_failure_account_body_v1(
            data,
            registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
            registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION,
            FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2,
        )
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        Ok(Some(CanonicalAccountProjection::contextual(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureExternalRoot,
            "failure adapter root digest and account-key authentication",
        )))
    } else if tag_version(
        data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
    ) && data.len() == FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1
    {
        let framed = decode_failure_account_body_v1(
            data,
            registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
            registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
            FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
        )
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let _ = RuntimeLivenessPolicyV1::decode(framed.body)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        Ok(Some(CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureLivenessPolicy,
        )))
    } else if tag_version(
        data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
    ) && data.len() == FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1
    {
        let framed = decode_failure_account_body_v1(
            data,
            registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
            registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
            FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
        )
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let value = RuntimeCompartmentV1::decode(framed.body)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureRecoveryCompartment,
        );
        projection.generation = Some(value.identity.generation);
        projection.keeper_hint = (value.phase == RuntimeCompartmentPhaseV1::Active
            && value.remaining_calls > 0)
            .then_some(KeeperHint {
                lane: None,
                position: WorkflowPosition {
                    phase: 1,
                    item: u64::from(value.completed_calls),
                },
                action: "advance-failure-recovery",
            });
        Ok(Some(projection))
    } else if tag_version(
        data,
        registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG,
        registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION,
    ) {
        let value = FailureReplayTombstoneV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureReplayTombstone,
        );
        projection.generation = Some(value.generation);
        projection.primary_binding = Some(value.market_instance_v2_id);
        Ok(Some(projection))
    } else {
        Ok(None)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForkNode {
    pub slot: u64,
    pub parent_slot: u64,
    pub blockhash: String,
    pub previous_blockhash: String,
    pub receive_sequence: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ForkLedger {
    nodes: BTreeMap<String, ForkNode>,
    hashes_by_slot: BTreeMap<u64, BTreeSet<String>>,
    frozen_slots: BTreeSet<u64>,
    dead_slots: BTreeSet<u64>,
    finalized_root: Option<(u64, String)>,
}

impl ForkLedger {
    pub fn observe(&mut self, slot: ObservedSlot, expected_cluster: &str) -> Result<()> {
        if slot.cluster_key != expected_cluster
            || slot.commitment != RpcCommitment::Processed
            || slot.slot == 0
            || slot.parent_slot >= slot.slot
            || slot.blockhash == slot.previous_blockhash
            || self.dead_slots.contains(&slot.slot)
        {
            return Err(AccountIndexError::InvalidFork);
        }
        if let Some(existing) = self.nodes.get(&slot.blockhash) {
            if existing.slot != slot.slot
                || existing.parent_slot != slot.parent_slot
                || existing.previous_blockhash != slot.previous_blockhash
            {
                return Err(AccountIndexError::InvalidFork);
            }
            return Ok(());
        }
        if self
            .nodes
            .get(&slot.previous_blockhash)
            .is_some_and(|parent| parent.slot != slot.parent_slot)
            || self.nodes.values().any(|child| {
                child.previous_blockhash == slot.blockhash && child.parent_slot != slot.slot
            })
        {
            return Err(AccountIndexError::InvalidFork);
        }
        self.hashes_by_slot
            .entry(slot.slot)
            .or_default()
            .insert(slot.blockhash.clone());
        self.nodes.insert(
            slot.blockhash.clone(),
            ForkNode {
                slot: slot.slot,
                parent_slot: slot.parent_slot,
                blockhash: slot.blockhash,
                previous_blockhash: slot.previous_blockhash,
                receive_sequence: slot.receive_sequence,
            },
        );
        Ok(())
    }

    pub fn observe_update(
        &mut self,
        update: ObservedSlotUpdate,
        expected_cluster: &str,
    ) -> Result<()> {
        if update.cluster_key != expected_cluster
            || update.slot == 0
            || update
                .parent_slot
                .is_some_and(|parent| parent >= update.slot)
        {
            return Err(AccountIndexError::InvalidFork);
        }
        match update.kind {
            ObservedSlotUpdateKind::Frozen => {
                if self.dead_slots.contains(&update.slot) {
                    return Err(AccountIndexError::InvalidFork);
                }
                self.frozen_slots.insert(update.slot);
            }
            ObservedSlotUpdateKind::Dead => {
                let conflicts_with_root = self.finalized_root.as_ref().is_some_and(|(_, root)| {
                    self.hashes_by_slot.get(&update.slot).is_some_and(|hashes| {
                        hashes.iter().any(|hash| self.is_ancestor(hash, root))
                    })
                });
                if conflicts_with_root {
                    return Err(AccountIndexError::InvalidFork);
                }
                self.frozen_slots.remove(&update.slot);
                self.dead_slots.insert(update.slot);
            }
            ObservedSlotUpdateKind::FirstShred
            | ObservedSlotUpdateKind::Completed
            | ObservedSlotUpdateKind::CreatedBank
            | ObservedSlotUpdateKind::OptimisticConfirmation
            | ObservedSlotUpdateKind::Root => {}
        }
        Ok(())
    }

    pub fn unique_hash_at(&self, slot: u64) -> Result<&str> {
        let hashes = self
            .hashes_by_slot
            .get(&slot)
            .ok_or(AccountIndexError::UnknownFork)?;
        if hashes.len() != 1 {
            return Err(AccountIndexError::AmbiguousFork);
        }
        hashes
            .first()
            .map(String::as_str)
            .ok_or(AccountIndexError::UnknownFork)
    }

    pub fn finalize_root(&mut self, slot: u64) -> Result<()> {
        if self
            .finalized_root
            .as_ref()
            .is_some_and(|(root, _)| slot < *root)
        {
            return Err(AccountIndexError::RootRegression);
        }
        let hash = self.unique_hash_at(slot)?.to_string();
        if self
            .finalized_root
            .as_ref()
            .is_some_and(|(_, prior)| !self.is_ancestor(prior, &hash))
        {
            return Err(AccountIndexError::InvalidFork);
        }
        if self.branch_contains_dead_slot(&hash) {
            return Err(AccountIndexError::InvalidFork);
        }
        self.finalized_root = Some((slot, hash));
        Ok(())
    }

    #[must_use]
    pub fn finalized_root(&self) -> Option<(u64, &str)> {
        self.finalized_root
            .as_ref()
            .map(|(slot, hash)| (*slot, hash.as_str()))
    }

    #[must_use]
    pub fn nodes(&self) -> Vec<&ForkNode> {
        self.nodes.values().collect()
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn contains_hash(&self, blockhash: &str) -> bool {
        self.nodes.contains_key(blockhash)
    }

    #[must_use]
    pub fn tracked_status_slot_count(&self) -> usize {
        self.frozen_slots.len() + self.dead_slots.len()
    }

    #[must_use]
    pub fn frozen_slots(&self) -> Vec<u64> {
        self.frozen_slots.iter().copied().collect()
    }

    #[must_use]
    pub fn is_frozen(&self, slot: u64) -> bool {
        self.frozen_slots.contains(&slot)
    }

    #[must_use]
    pub fn is_dead(&self, slot: u64) -> bool {
        self.dead_slots.contains(&slot)
    }

    #[must_use]
    pub fn slot_is_on_dead_branch(&self, slot: u64) -> bool {
        self.hashes_by_slot.get(&slot).is_some_and(|hashes| {
            hashes
                .iter()
                .any(|hash| self.branch_contains_dead_slot(hash))
        })
    }

    #[must_use]
    pub fn dead_slots(&self) -> Vec<u64> {
        self.dead_slots.iter().copied().collect()
    }

    fn is_ancestor(&self, ancestor: &str, descendant: &str) -> bool {
        let mut cursor = descendant;
        let mut remaining = self.nodes.len();
        while remaining > 0 {
            if cursor == ancestor {
                return true;
            }
            let Some(node) = self.nodes.get(cursor) else {
                return false;
            };
            cursor = &node.previous_blockhash;
            remaining -= 1;
        }
        false
    }

    fn branch_contains_dead_slot(&self, descendant: &str) -> bool {
        let mut cursor = descendant;
        let mut remaining = self.nodes.len();
        while remaining > 0 {
            let Some(node) = self.nodes.get(cursor) else {
                return false;
            };
            if self.dead_slots.contains(&node.slot) {
                return true;
            }
            cursor = &node.previous_blockhash;
            remaining -= 1;
        }
        false
    }

    fn processed_tip(&self) -> Option<&str> {
        self.nodes
            .values()
            .filter(|node| match self.finalized_root.as_ref() {
                Some((_, root)) => {
                    self.is_ancestor(root, &node.blockhash)
                        && !self.branch_contains_dead_slot(&node.blockhash)
                        && self.frozen_slots.contains(&node.slot)
                }
                None => {
                    !self.branch_contains_dead_slot(&node.blockhash)
                        && self.frozen_slots.contains(&node.slot)
                }
            })
            .max_by(|left, right| {
                (left.slot, left.receive_sequence, &left.blockhash).cmp(&(
                    right.slot,
                    right.receive_sequence,
                    &right.blockhash,
                ))
            })
            .map(|node| node.blockhash.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexedBranch {
    FinalizedScan,
    Processed { blockhash: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedAccountVersion {
    pub account: ObservedRpcAccount,
    pub projection: CanonicalAccountProjection,
    pub data_sha256: [u8; 32],
    pub branch: IndexedBranch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedAccountAbsence {
    pub release_key: String,
    pub slot: u64,
    pub receive_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexCapacity {
    pub maximum_addresses: usize,
    pub maximum_versions_per_address: usize,
    pub maximum_fork_nodes: usize,
}

#[derive(Clone)]
pub struct CanonicalAccountIndex {
    plan: RpcIndexPlan,
    context: CanonicalDecoderContext,
    capacity: IndexCapacity,
    forks: ForkLedger,
    versions: BTreeMap<Address, Vec<IndexedAccountVersion>>,
    finalized_absences: BTreeMap<Address, FinalizedAccountAbsence>,
}

impl CanonicalAccountIndex {
    pub fn new(
        plan: RpcIndexPlan,
        context: CanonicalDecoderContext,
        capacity: IndexCapacity,
    ) -> Result<Self> {
        plan.validate()
            .map_err(|_| AccountIndexError::UnknownRelease)?;
        if capacity.maximum_addresses == 0
            || capacity.maximum_versions_per_address == 0
            || capacity.maximum_fork_nodes == 0
        {
            return Err(AccountIndexError::CapacityExceeded);
        }
        Ok(Self {
            plan,
            context,
            capacity,
            forks: ForkLedger::default(),
            versions: BTreeMap::new(),
            finalized_absences: BTreeMap::new(),
        })
    }

    pub fn observe_slot(&mut self, slot: ObservedSlot) -> Result<()> {
        if !self.forks.contains_hash(&slot.blockhash)
            && self.forks.node_count() >= self.capacity.maximum_fork_nodes
        {
            return Err(AccountIndexError::CapacityExceeded);
        }
        self.forks.observe(slot, &self.plan.cluster.key())
    }

    pub fn observe_slot_update(&mut self, update: ObservedSlotUpdate) -> Result<()> {
        if matches!(
            update.kind,
            ObservedSlotUpdateKind::Frozen | ObservedSlotUpdateKind::Dead
        ) && !self.forks.is_frozen(update.slot)
            && !self.forks.is_dead(update.slot)
            && self.forks.tracked_status_slot_count() >= self.capacity.maximum_fork_nodes
        {
            return Err(AccountIndexError::CapacityExceeded);
        }
        self.forks.observe_update(update, &self.plan.cluster.key())
    }

    pub fn finalize_root(&mut self, slot: u64) -> Result<()> {
        self.forks.finalize_root(slot)
    }

    /// Invalidate every non-final subscription observation after a transport
    /// disconnect. Finalized scans remain available, while fork topology and
    /// processed versions are rebuilt only from a complete new subscription
    /// generation. Rooted subscription rows are deliberately dropped too:
    /// the next finalized scan, not a disconnected feed, owns their promotion.
    pub fn rollback_processed_transport(&mut self) -> usize {
        let mut removed = 0_usize;
        self.versions.retain(|_, versions| {
            let before = versions.len();
            versions.retain(|version| matches!(&version.branch, IndexedBranch::FinalizedScan));
            removed = removed.saturating_add(before.saturating_sub(versions.len()));
            !versions.is_empty()
        });
        self.forks = ForkLedger::default();
        removed
    }

    /// Remove already-indexed rows on every branch whose ancestry now contains
    /// a dead slot. Finalized-scan baselines are never removed by a processed
    /// rollback observation.
    pub fn rollback_dead_processed_versions(&mut self) -> usize {
        let forks = &self.forks;
        let mut removed = 0_usize;
        self.versions.retain(|_, versions| {
            let before = versions.len();
            versions.retain(|version| match &version.branch {
                IndexedBranch::FinalizedScan => true,
                IndexedBranch::Processed { blockhash } => {
                    !forks.branch_contains_dead_slot(blockhash)
                }
            });
            removed = removed.saturating_add(before.saturating_sub(versions.len()));
            !versions.is_empty()
        });
        removed
    }

    pub fn ingest(&mut self, account: ObservedRpcAccount) -> Result<()> {
        let branch = match account.provenance.commitment {
            RpcCommitment::Finalized => IndexedBranch::FinalizedScan,
            RpcCommitment::Processed => IndexedBranch::Processed {
                blockhash: self
                    .forks
                    .unique_hash_at(account.provenance.slot)?
                    .to_string(),
            },
        };
        let projection =
            CanonicalAccountDecoderRegistry::new(&self.plan, self.context).decode(&account)?;
        let data_sha256 = Sha256::digest(&account.data).into();
        if !self.versions.contains_key(&account.address)
            && self.versions.len() >= self.capacity.maximum_addresses
        {
            return Err(AccountIndexError::CapacityExceeded);
        }
        let versions = self.versions.entry(account.address).or_default();
        if versions.last().is_some_and(|previous| {
            previous.account.provenance.receive_sequence >= account.provenance.receive_sequence
                && previous.account.provenance.slot >= account.provenance.slot
        }) {
            return Err(AccountIndexError::StaleObservation);
        }
        if versions.len() >= self.capacity.maximum_versions_per_address {
            versions.remove(0);
        }
        versions.push(IndexedAccountVersion {
            account,
            projection,
            data_sha256,
            branch,
        });
        Ok(())
    }

    pub fn reconcile_finalized_scan(
        &mut self,
        release_key: &str,
        slot: u64,
        receive_sequence: u64,
        seen: &BTreeSet<Address>,
    ) -> Result<()> {
        if self.release(release_key).is_none() {
            return Err(AccountIndexError::UnknownRelease);
        }
        for (address, versions) in &self.versions {
            let belongs_to_release = versions
                .iter()
                .any(|version| version.account.provenance.release_key == release_key);
            if belongs_to_release && !seen.contains(address) {
                let absence = FinalizedAccountAbsence {
                    release_key: release_key.to_string(),
                    slot,
                    receive_sequence,
                };
                let should_replace = self.finalized_absences.get(address).is_none_or(|prior| {
                    (prior.slot, prior.receive_sequence) < (absence.slot, absence.receive_sequence)
                });
                if should_replace {
                    self.finalized_absences.insert(*address, absence);
                }
            }
        }
        Ok(())
    }

    pub fn current(
        &self,
        address: Address,
        commitment: RpcCommitment,
    ) -> Option<&IndexedAccountVersion> {
        let versions = self.versions.get(&address)?;
        let selected = match commitment {
            RpcCommitment::Processed => {
                let tip = self.forks.processed_tip();
                versions
                    .iter()
                    .filter(|version| match (&version.branch, tip) {
                        (IndexedBranch::FinalizedScan, _) => true,
                        (IndexedBranch::Processed { blockhash }, Some(tip)) => {
                            self.forks.is_ancestor(blockhash, tip)
                        }
                        _ => false,
                    })
                    .max_by_key(|version| {
                        (
                            version.account.provenance.slot,
                            version.account.provenance.receive_sequence,
                        )
                    })
            }
            RpcCommitment::Finalized => versions
                .iter()
                .filter(|version| matches!(&version.branch, IndexedBranch::FinalizedScan))
                .max_by_key(|version| {
                    (
                        version.account.provenance.slot,
                        version.account.provenance.receive_sequence,
                    )
                }),
        };
        selected.filter(|version| {
            self.finalized_absences.get(&address).is_none_or(|absence| {
                absence.release_key != version.account.provenance.release_key
                    || version.account.provenance.slot > absence.slot
            })
        })
    }

    pub fn current_accounts(&self, commitment: RpcCommitment) -> Vec<&IndexedAccountVersion> {
        self.versions
            .keys()
            .filter_map(|address| self.current(*address, commitment))
            .collect()
    }

    #[must_use]
    pub fn finalized_absence(&self, address: Address) -> Option<&FinalizedAccountAbsence> {
        self.finalized_absences.get(&address)
    }

    #[must_use]
    pub const fn forks(&self) -> &ForkLedger {
        &self.forks
    }

    #[must_use]
    pub fn release(&self, key: &str) -> Option<&IndexedProgramRelease> {
        self.plan
            .releases
            .iter()
            .find(|release| release.key() == key)
    }

    #[must_use]
    pub fn releases(&self) -> &[IndexedProgramRelease] {
        &self.plan.releases
    }

    #[must_use]
    pub const fn acquisition_plan(&self) -> &RpcIndexPlan {
        &self.plan
    }

    #[must_use]
    pub fn cluster_key(&self) -> String {
        self.plan.cluster.key()
    }
}

#[cfg(test)]
mod processed_fork_tests {
    use super::*;

    const CLUSTER: &str = "test:genesis";

    fn slot(slot: u64, parent_slot: u64, blockhash: &str, previous: &str) -> ObservedSlot {
        ObservedSlot {
            cluster_key: CLUSTER.to_string(),
            slot,
            parent_slot,
            blockhash: blockhash.to_string(),
            previous_blockhash: previous.to_string(),
            commitment: RpcCommitment::Processed,
            receive_sequence: slot,
        }
    }

    #[test]
    fn later_root_must_descend_from_the_previous_root() {
        let mut ledger = ForkLedger::default();
        ledger
            .observe(slot(10, 9, "root-10", "unknown-9"), CLUSTER)
            .unwrap();
        ledger.finalize_root(10).unwrap();
        ledger
            .observe(slot(12, 11, "orphan-12", "orphan-11"), CLUSTER)
            .unwrap();
        assert_eq!(
            ledger.finalize_root(12),
            Err(AccountIndexError::InvalidFork)
        );
        let mut valid = ForkLedger::default();
        valid
            .observe(slot(10, 9, "root-10", "unknown-9"), CLUSTER)
            .unwrap();
        valid.finalize_root(10).unwrap();
        valid
            .observe(slot(11, 10, "child-11", "root-10"), CLUSTER)
            .unwrap();
        valid
            .observe(slot(12, 11, "child-12", "child-11"), CLUSTER)
            .unwrap();
        assert_eq!(valid.finalize_root(12), Ok(()));
    }

    #[test]
    fn dead_ancestor_marks_its_known_descendants_rollbackable() {
        let mut ledger = ForkLedger::default();
        ledger
            .observe(slot(20, 19, "branch-20", "unknown-19"), CLUSTER)
            .unwrap();
        ledger
            .observe(slot(21, 20, "branch-21", "branch-20"), CLUSTER)
            .unwrap();
        ledger
            .observe_update(
                ObservedSlotUpdate {
                    cluster_key: CLUSTER.to_string(),
                    slot: 20,
                    parent_slot: Some(19),
                    kind: ObservedSlotUpdateKind::Dead,
                    receive_sequence: 22,
                },
                CLUSTER,
            )
            .unwrap();
        assert!(ledger.slot_is_on_dead_branch(20));
        assert!(ledger.slot_is_on_dead_branch(21));
    }
}
