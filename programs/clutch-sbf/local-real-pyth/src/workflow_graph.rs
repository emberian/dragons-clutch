//! Deterministic, resumable construction graph for real protocol operations.
//!
//! Every planner consumes a freshly observed canonical state, an explicit
//! release manifest, and semantic-owner payload bytes. It returns one unsigned
//! transaction plus a reload barrier. It has no keypair, RPC, clock discovery,
//! signing, submission, or optimistic post-state projection.

use crate::transaction_builder::{
    ExactEquation, OwnedInstructionDraft, ProtocolFlow, ProtocolTransactionBuilder, SemanticOwner,
    UnsignedProtocolTransaction,
};
use clutch_general_v2_contract::{
    claim_solver_poststate_v1, cleanup_candidate_poststate_v1, close_clear_work_poststate_v1,
    decode_direct_settlement_payload_v1, decode_identity_lab_payload_v1,
    decode_owner_settlement_payload_v1, decode_virtual_settlement_payload_v1,
    expire_committed_candidate_poststate_v1, AdmissionNodeStatusV1, AdmissionNodeV3AccountV1,
    CandidateBundleRetirementContractV1, CandidateFeedHeaderV2, CandidateWindowV4AccountV1,
    ClaimSolverTransitionV1, CleanupCandidateTransitionV1, ClearWorkHeaderV2,
    CloseClearWorkTransitionV1, EpochBudgetV2AccountV1, ExpireCommittedCandidateTransitionV1,
    GeneralEpochPhaseV1, GeneralEpochV6AccountV1, IdentityLabPayloadV1, MarketRuntimeV3AccountV1,
    OwnerSettlementPayloadV1, SelectedCandidateRetirementContractV1, SelectedCandidateV1AccountV1,
    WriteCandidateFeedPayloadV1,
};
use clutch_owner_settlement::{
    prepare_account_receipt_end_v1, prepare_direct_egg_settlement_v1,
    prepare_realize_owner_cash_v1, prepare_virtual_merge_receipt_v1,
    prepare_virtual_split_receipt_v1, AuthenticatedOwnerFeeDebitV1,
    AuthenticatedOwnerSettlementAccountV1, AuthenticatedPositionCashV1,
    AuthenticatedSettlementReceiptEndV1, DirectEggSettlementInputV1, OwnerSettlementAccumulatorV1,
    SettlementCashPotV1, VirtualMergeReceiptInputV1, VirtualSplitReceiptInputV1,
};
use clutch_product_series::{SeriesFundingQuoteV1, SeriesFundingTermsV2};
use clutch_retirement::PositionAccountV3;
use clutch_solana_layout::product_series::{
    ActivateSeriesFundingIntentV1, AdvanceSeriesOccurrenceIntentV1, CloseSeriesFundingIntentV1,
    LapseSeriesOccurrenceIntentV1, ObserveSeriesDonationIntentV1, RegisterSeriesIntentV1,
    SeriesFundingAccountV1, SeriesRegistryAccountV1, ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1,
    ADVANCE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1, CLOSE_SERIES_FUNDING_PAYLOAD_BYTES_V1,
    LAPSE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1, OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1,
    REGISTER_SERIES_PAYLOAD_BYTES_V1,
};
use clutch_solana_layout::registry::{
    ExtensionAction, GeneralV2Action, RecurringSeriesAction, SourceSeriesAction,
};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_adapter::{
    IntentPreimageV3, TransitionActionV3, TransitionPlanV3, INTENT_PREIMAGE_BYTES,
};
use clutch_source_plane_v3_runtime::{ReopenLineageV1, SourceReleaseManifestV1};
use clutch_structured_claim_runtime_contract::{
    decode_structured_claim_payload_v1, DescriptorStateV1, StructuredClaimActionV1,
    StructuredClaimDescriptorV1,
};
use solana_address::Address;
use solana_instruction::AccountMeta;
use std::collections::BTreeSet;

pub type Result<T> = core::result::Result<T, WorkflowGraphError>;

/// Refusals from the construction graph. None is an execution receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowGraphError {
    ZeroIdentity,
    DuplicateRelease,
    UnknownSemanticRelease,
    WrongProgramRelease,
    WrongCursor,
    WrongLane,
    InvalidCanonicalState,
    InvalidCanonicalPayload,
    ActionStateMismatch,
    NotReady,
    Construction,
}

impl core::fmt::Display for WorkflowGraphError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ZeroIdentity => "workflow graph contains a zero identity",
            Self::DuplicateRelease => "release manifest contains a duplicate semantic owner",
            Self::UnknownSemanticRelease => "action semantic owner is absent from the manifest",
            Self::WrongProgramRelease => "action targets the wrong released program",
            Self::WrongCursor => "resumable cursor does not match canonical observed progress",
            Self::WrongLane => "resumable cursor belongs to another workflow lane",
            Self::InvalidCanonicalState => "canonical account state refused the workflow step",
            Self::InvalidCanonicalPayload => "semantic owner refused the action payload",
            Self::ActionStateMismatch => "payload action does not match canonical observed state",
            Self::NotReady => "canonical dependencies are not terminal for this step",
            Self::Construction => "unsigned outer transaction construction refused the step",
        })
    }
}

impl std::error::Error for WorkflowGraphError {}

/// One exact program release named by the explicit operator manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleasedProgram {
    pub program_id: Address,
    pub program_data: Address,
    pub deployment_slot: u64,
    pub elf_sha256: [u8; 32],
}

impl ReleasedProgram {
    fn validate(self) -> Result<()> {
        if self.program_id == Address::default()
            || self.program_data == Address::default()
            || self.program_id == self.program_data
            || self.deployment_slot == 0
            || self.elf_sha256 == [0; 32]
        {
            Err(WorkflowGraphError::ZeroIdentity)
        } else {
            Ok(())
        }
    }
}

/// Explicit releases allowed to own bytes in one workflow graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitOperatorReleaseManifest {
    pub manifest_sha256: [u8; 32],
    pub clutch: ReleasedProgram,
    /// Exact captured Pyth receiver release admitted by the Source release.
    pub pyth_receiver: ReleasedProgram,
    /// Exact captured Pyth router release used to authenticate VAA transport.
    pub pyth_router: ReleasedProgram,
    pub semantic_releases: Vec<SemanticOwner>,
}

impl ExplicitOperatorReleaseManifest {
    pub fn validate(&self) -> Result<()> {
        if self.manifest_sha256 == [0; 32] {
            return Err(WorkflowGraphError::ZeroIdentity);
        }
        self.clutch.validate()?;
        self.pyth_receiver.validate()?;
        self.pyth_router.validate()?;
        if self.pyth_receiver.program_id == self.pyth_router.program_id
            || self.pyth_receiver.program_data == self.pyth_router.program_data
            || self.pyth_receiver.program_id == self.clutch.program_id
            || self.pyth_router.program_id == self.clutch.program_id
            || self.semantic_releases.is_empty()
        {
            return Err(WorkflowGraphError::WrongProgramRelease);
        }
        let mut releases = BTreeSet::new();
        for release in &self.semantic_releases {
            release
                .validate()
                .map_err(|_| WorkflowGraphError::ZeroIdentity)?;
            if !releases.insert((release.package.clone(), release.schema.clone())) {
                return Err(WorkflowGraphError::DuplicateRelease);
            }
        }
        Ok(())
    }

    fn admits_owner(&self, owner: &SemanticOwner) -> Result<()> {
        self.validate()?;
        if self
            .semantic_releases
            .iter()
            .any(|release| release == owner)
        {
            Ok(())
        } else {
            Err(WorkflowGraphError::UnknownSemanticRelease)
        }
    }
}

/// Independent resumable lanes in the operator graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkflowLane {
    Creation,
    SourceCrank,
    Candidate,
    KeeperReceipts,
    RecoveryRetirement,
}

/// Deterministic cursor position derived from canonical account progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkflowPosition {
    pub phase: u16,
    pub item: u64,
}

/// Durable resume input. `observed_state_sha256` identifies the exact account
/// snapshot from which this position was derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResumableWorkflowCursor {
    pub workflow_id: [u8; 32],
    pub lane: WorkflowLane,
    pub generation: u64,
    pub position: WorkflowPosition,
    pub observed_state_sha256: [u8; 32],
}

impl ResumableWorkflowCursor {
    fn require(
        self,
        lane: WorkflowLane,
        generation: u64,
        position: WorkflowPosition,
        observed_state_sha256: [u8; 32],
    ) -> Result<()> {
        if self.workflow_id == [0; 32]
            || self.observed_state_sha256 == [0; 32]
            || observed_state_sha256 == [0; 32]
            || self.generation == 0
        {
            return Err(WorkflowGraphError::ZeroIdentity);
        }
        if self.lane != lane {
            return Err(WorkflowGraphError::WrongLane);
        }
        if self.generation != generation
            || self.position != position
            || self.observed_state_sha256 != observed_state_sha256
        {
            return Err(WorkflowGraphError::WrongCursor);
        }
        Ok(())
    }
}

/// Bytes and Solana metas still owned by the semantic action implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowActionMaterial {
    pub action_name: String,
    pub semantic_owner: SemanticOwner,
    pub accounts: Vec<AccountMeta>,
    pub required_signers: Vec<Address>,
    pub exact_equations: Vec<ExactEquation>,
    pub payload: Vec<u8>,
}

/// Source-specific material whose bytes are derived from a guarded transition
/// plan rather than accepted as a caller-authored payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceWorkflowActionMaterial {
    pub action_name: String,
    pub semantic_owner: SemanticOwner,
    pub accounts: Vec<AccountMeta>,
    pub required_signers: Vec<Address>,
    pub exact_equations: Vec<ExactEquation>,
    pub transition_plan: TransitionPlanV3,
    pub submitter: ContentId,
    pub valid_before_slot: u64,
}

/// One construction result and its mandatory fresh-state reload barrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedWorkflowNode {
    pub manifest_sha256: [u8; 32],
    pub cursor: ResumableWorkflowCursor,
    pub coordinate: CanonicalActionCoordinate,
    pub unsigned_transaction: UnsignedProtocolTransaction,
    pub reload_authoritative_accounts: bool,
}

/// Registry and semantic-owner coordinate selected by canonical state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalActionCoordinate {
    General(GeneralV2Action),
    SourceRegistry(SourceSeriesAction),
    SourceTransition {
        registry: SourceSeriesAction,
        transition: TransitionActionV3,
    },
    Series(RecurringSeriesAction),
    StructuredClaim(StructuredClaimActionV1),
}

/// Account absence observed by an untrusted reader. It can drive construction,
/// never authorization inside the eventual program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbsentAccountObservation {
    pub address: Address,
    pub observed_state_sha256: [u8; 32],
}

impl AbsentAccountObservation {
    fn validate(self) -> Result<()> {
        if self.address == Address::default() || self.observed_state_sha256 == [0; 32] {
            Err(WorkflowGraphError::ZeroIdentity)
        } else {
            Ok(())
        }
    }
}

/// Market creation is a distinct graph root because action 1's payload remains
/// owned by the market adapter rather than a second operator DTO.
pub fn plan_market_creation(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    absence: AbsentAccountObservation,
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    absence.validate()?;
    cursor.require(
        WorkflowLane::Creation,
        1,
        WorkflowPosition { phase: 1, item: 0 },
        absence.observed_state_sha256,
    )?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::MarketEpochCreation,
        CanonicalActionCoordinate::General(GeneralV2Action::CreateMarket),
        material,
    )
}

/// Create exactly the next Epoch from the canonical MarketRuntime cursor.
pub fn plan_epoch_creation(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    market: MarketRuntimeV3AccountV1,
    epoch_absence: AbsentAccountObservation,
    observed_state_sha256: [u8; 32],
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    market
        .validate()
        .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
    epoch_absence.validate()?;
    let intent =
        match decode_identity_lab_payload_v1(GeneralV2Action::InitEpoch.tag(), &material.payload)
            .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?
        {
            IdentityLabPayloadV1::InitEpoch(intent) => intent,
            _ => return Err(WorkflowGraphError::ActionStateMismatch),
        };
    if intent.market_instance_v2_id != market.market_instance_v2_id
        || intent.epoch_index != market.next_epoch_index
    {
        return Err(WorkflowGraphError::ActionStateMismatch);
    }
    cursor.require(
        WorkflowLane::Creation,
        market.next_epoch_generation,
        WorkflowPosition {
            phase: 2,
            item: market.next_epoch_index,
        },
        observed_state_sha256,
    )?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::MarketEpochCreation,
        CanonicalActionCoordinate::General(GeneralV2Action::InitEpoch),
        material,
    )
}

/// Register a V5 Series at its canonical absent account.
pub fn plan_series_registration(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    absence: AbsentAccountObservation,
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    absence.validate()?;
    cursor.require(
        WorkflowLane::Creation,
        1,
        WorkflowPosition { phase: 3, item: 0 },
        absence.observed_state_sha256,
    )?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::ProductSeries,
        CanonicalActionCoordinate::Series(RecurringSeriesAction::RegisterSeries),
        material,
    )
}

/// Activate the one permitted Series funding generation.
pub fn plan_series_funding_activation(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    registry: &SeriesRegistryAccountV1,
    funding_absence: AbsentAccountObservation,
    observed_state_sha256: [u8; 32],
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    registry
        .validate()
        .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
    funding_absence.validate()?;
    if registry.activation_consumed {
        return Err(WorkflowGraphError::NotReady);
    }
    let intent = ActivateSeriesFundingIntentV1::decode(&material.payload)
        .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
    if intent.series_plan_id != registry.series_plan_id {
        return Err(WorkflowGraphError::ActionStateMismatch);
    }
    cursor.require(
        WorkflowLane::Creation,
        1,
        WorkflowPosition { phase: 4, item: 0 },
        observed_state_sha256,
    )?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::ProductSeries,
        CanonicalActionCoordinate::Series(RecurringSeriesAction::ActivateFunding),
        material,
    )
}

/// Create/converge or lapse exactly the funding state's next Series ordinal.
pub fn plan_series_occurrence(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    funding: &SeriesFundingAccountV1,
    lapse: bool,
    observed_state_sha256: [u8; 32],
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    funding
        .validate()
        .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
    if funding.state.next_ordinal >= funding.state.instance_count {
        return Err(WorkflowGraphError::NotReady);
    }
    let action = if lapse {
        RecurringSeriesAction::LapseOccurrence
    } else {
        RecurringSeriesAction::AdvanceOccurrence
    };
    if lapse {
        let intent = LapseSeriesOccurrenceIntentV1::decode(&material.payload)
            .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
        if intent.series_plan_id != funding.state.series_plan_id
            || intent.ordinal != funding.state.next_ordinal
        {
            return Err(WorkflowGraphError::ActionStateMismatch);
        }
    } else {
        let intent = AdvanceSeriesOccurrenceIntentV1::decode(&material.payload)
            .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
        if intent.series_plan_id != funding.state.series_plan_id
            || intent.ordinal != funding.state.next_ordinal
        {
            return Err(WorkflowGraphError::ActionStateMismatch);
        }
    }
    cursor.require(
        WorkflowLane::Creation,
        1,
        WorkflowPosition {
            phase: if lapse { 7 } else { 6 },
            item: u64::from(funding.state.next_ordinal),
        },
        observed_state_sha256,
    )?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::ProductSeries,
        CanonicalActionCoordinate::Series(action),
        material,
    )
}

/// Create one immutable structured-claim descriptor through its canonical
/// semantic-owner action codec.
pub fn plan_descriptor_creation(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    absence: AbsentAccountObservation,
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    absence.validate()?;
    cursor.require(
        WorkflowLane::Creation,
        1,
        WorkflowPosition { phase: 5, item: 0 },
        absence.observed_state_sha256,
    )?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::StructuredClaim,
        CanonicalActionCoordinate::StructuredClaim(StructuredClaimActionV1::CreateDescriptor),
        material,
    )
}

/// Exhaustive Source ingest/window/evaluation stages. The stage is equality-
/// checked against the exact canonical `IntentPreimageV3` action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCrankStage {
    InitializeHead,
    OpenRawPage { page_index: u64 },
    IngestBoundary { boundary_bucket: u64 },
    SealRawPage { page_index: u64 },
    InitializeWindow { window_ordinal: u64 },
    FoldWindowPage { page_index: u64 },
    SealWindow { window_ordinal: u64 },
    EvaluateTerminal { window_ordinal: u64 },
    EvaluateDrawdown { window_ordinal: u64 },
}

impl SourceCrankStage {
    fn coordinate(self) -> (WorkflowPosition, SourceSeriesAction, TransitionActionV3) {
        match self {
            Self::InitializeHead => (
                WorkflowPosition { phase: 1, item: 0 },
                SourceSeriesAction::InitializeHead,
                TransitionActionV3::InitializeSourceHead,
            ),
            Self::OpenRawPage { page_index } => (
                WorkflowPosition {
                    phase: 2,
                    item: page_index,
                },
                SourceSeriesAction::OpenRawPage,
                TransitionActionV3::OpenRawPage,
            ),
            Self::IngestBoundary { boundary_bucket } => (
                WorkflowPosition {
                    phase: 3,
                    item: boundary_bucket,
                },
                SourceSeriesAction::IngestBoundaryBatch,
                TransitionActionV3::AppendBoundary,
            ),
            Self::SealRawPage { page_index } => (
                WorkflowPosition {
                    phase: 4,
                    item: page_index,
                },
                SourceSeriesAction::SealRawPage,
                TransitionActionV3::SealRawPage,
            ),
            Self::InitializeWindow { window_ordinal } => (
                WorkflowPosition {
                    phase: 5,
                    item: window_ordinal,
                },
                SourceSeriesAction::InitializeWindowWork,
                TransitionActionV3::CreateWindowWork,
            ),
            Self::FoldWindowPage { page_index } => (
                WorkflowPosition {
                    phase: 6,
                    item: page_index,
                },
                SourceSeriesAction::FoldWindowPages,
                TransitionActionV3::FoldWindowPage,
            ),
            Self::SealWindow { window_ordinal } => (
                WorkflowPosition {
                    phase: 7,
                    item: window_ordinal,
                },
                SourceSeriesAction::SealWindow,
                TransitionActionV3::SealWindow,
            ),
            Self::EvaluateTerminal { window_ordinal } => (
                WorkflowPosition {
                    phase: 8,
                    item: window_ordinal,
                },
                SourceSeriesAction::EvaluateStatistic,
                TransitionActionV3::WriteTerminalResult,
            ),
            Self::EvaluateDrawdown { window_ordinal } => (
                WorkflowPosition {
                    phase: 9,
                    item: window_ordinal,
                },
                SourceSeriesAction::EvaluateStatistic,
                TransitionActionV3::WriteDrawdownResult,
            ),
        }
    }
}

/// The state a semantic Source transition requires from one persisted lineage.
/// This is a precondition over the canonical lineage codec, not a second copy
/// of lineage state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLineageExpectation {
    NeverCreated,
    OpenAtGeneration(u64),
    ClosedAtGeneration(u64),
}

/// One canonical lineage account and the exact state required by the planned
/// transition. Callers include every mutable Source family the intent reads or
/// writes; the graph refuses empty or aliased observations.
#[derive(Clone, Copy, Debug)]
pub struct ObservedSourceLineage<'a> {
    pub lineage: &'a ReopenLineageV1,
    pub expectation: SourceLineageExpectation,
}

/// Canonical Source state from which one adapter intent was constructed.
#[derive(Clone, Copy, Debug)]
pub struct SourceCrankObservation<'a> {
    /// Canonical immutable release body selected by the operator. Execution
    /// independently authenticates its owner, content-addressed PDA and bytes.
    pub release: &'a SourceReleaseManifestV1,
    pub generation: u64,
    pub stage: SourceCrankStage,
    pub lineages: &'a [ObservedSourceLineage<'a>],
    pub observed_state_sha256: [u8; 32],
}

impl SourceCrankObservation<'_> {
    fn validate(self, manifest: &ExplicitOperatorReleaseManifest) -> Result<()> {
        if self.generation == 0 || self.observed_state_sha256 == [0; 32] || self.lineages.is_empty()
        {
            return Err(WorkflowGraphError::InvalidCanonicalState);
        }
        self.release
            .validate()
            .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
        if self.release.adapter.program.bytes() != manifest.clutch.program_id.to_bytes()
            || self.release.adapter.programdata.bytes() != manifest.clutch.program_data.to_bytes()
            || self.release.adapter.deployment_slot != manifest.clutch.deployment_slot
            || self.release.parser.program.bytes()
                != manifest.pyth_receiver.program_id.to_bytes()
            || self.release.parser.programdata.bytes()
                != manifest.pyth_receiver.program_data.to_bytes()
            || self.release.parser.deployment_slot != manifest.pyth_receiver.deployment_slot
        {
            return Err(WorkflowGraphError::WrongProgramRelease);
        }
        let mut accounts = BTreeSet::new();
        for observed in self.lineages {
            let lineage = observed.lineage;
            lineage
                .validate()
                .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
            if lineage.adapter_program.bytes() != manifest.clutch.program_id.to_bytes()
                || !accounts.insert(lineage.lineage_account.bytes())
            {
                return Err(WorkflowGraphError::InvalidCanonicalState);
            }
            let expected = match observed.expectation {
                SourceLineageExpectation::NeverCreated => {
                    lineage.latest_generation == 0 && !lineage.is_open
                }
                SourceLineageExpectation::OpenAtGeneration(generation) => {
                    generation == self.generation
                        && lineage.latest_generation == generation
                        && lineage.is_open
                }
                SourceLineageExpectation::ClosedAtGeneration(generation) => {
                    generation < self.generation
                        && lineage.latest_generation == generation
                        && !lineage.is_open
                }
            };
            if !expected {
                return Err(WorkflowGraphError::ActionStateMismatch);
            }
        }
        if self.stage == SourceCrankStage::InitializeHead
            && !self
                .lineages
                .iter()
                .any(|observed| observed.expectation == SourceLineageExpectation::NeverCreated)
        {
            return Err(WorkflowGraphError::ActionStateMismatch);
        }
        Ok(())
    }
}

pub fn plan_source_crank(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    observation: SourceCrankObservation<'_>,
    cursor: ResumableWorkflowCursor,
    material: SourceWorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    observation.validate(manifest)?;
    let (position, registry, transition) = observation.stage.coordinate();
    material
        .transition_plan
        .validate()
        .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
    if material.transition_plan.action() != transition
        || material.submitter.is_zero()
        || material.valid_before_slot == 0
    {
        return Err(WorkflowGraphError::ActionStateMismatch);
    }
    let adapter_program_id = ContentId::from_bytes(manifest.clutch.program_id.to_bytes());
    let intent = IntentPreimageV3::new(
        material.transition_plan,
        adapter_program_id,
        material.submitter,
        material.valid_before_slot,
    )
    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
    let payload = intent
        .encode()
        .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
    cursor.require(
        WorkflowLane::SourceCrank,
        observation.generation,
        position,
        observation.observed_state_sha256,
    )?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::SourcePlaneV3,
        CanonicalActionCoordinate::SourceTransition {
            registry,
            transition,
        },
        WorkflowActionMaterial {
            action_name: material.action_name,
            semantic_owner: material.semantic_owner,
            accounts: material.accounts,
            required_signers: material.required_signers,
            exact_equations: material.exact_equations,
            payload: payload.to_vec(),
        },
    )
}

/// Canonical account projection sufficient to derive the next candidate action.
#[derive(Clone, Copy, Debug)]
pub struct CandidateCrankObservation<'a> {
    pub epoch: &'a GeneralEpochV6AccountV1,
    pub window: &'a CandidateWindowV4AccountV1,
    pub node: Option<&'a AdmissionNodeV3AccountV1>,
    pub feed: Option<(&'a CandidateFeedHeaderV2, bool)>,
    pub work: Option<&'a ClearWorkHeaderV2>,
    pub selected: Option<&'a SelectedCandidateV1AccountV1>,
    pub observed_state_sha256: [u8; 32],
}

impl CandidateCrankObservation<'_> {
    fn next(self) -> Result<(WorkflowPosition, GeneralV2Action)> {
        if self.observed_state_sha256 == [0; 32] {
            return Err(WorkflowGraphError::InvalidCanonicalState);
        }
        self.epoch
            .validate()
            .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
        self.window
            .validate()
            .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
        let expected_epoch_phase = if self.selected.is_some() {
            GeneralEpochPhaseV1::Finalized
        } else {
            GeneralEpochPhaseV1::Frozen
        };
        if self.epoch.phase != expected_epoch_phase
            || self.window.market != self.epoch.market_runtime
            || self.window.epoch_generation != self.epoch.generation
        {
            return Err(WorkflowGraphError::InvalidCanonicalState);
        }
        if let Some(selected) = self.selected {
            selected
                .validate()
                .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
            if self.window.finalized_slot == 0
                || selected.epoch != self.window.epoch
                || selected.epoch_generation != self.epoch.generation
                || selected.market != self.window.market
                || selected.window != self.epoch.window
            {
                return Err(WorkflowGraphError::InvalidCanonicalState);
            }
            let item = u64::from(selected.next_slice_index);
            return match selected.entitlement_state {
                0 | 1 => Ok((
                    WorkflowPosition { phase: 10, item },
                    GeneralV2Action::FreezeEntitlement,
                )),
                2 => Err(WorkflowGraphError::NotReady),
                _ => Err(WorkflowGraphError::InvalidCanonicalState),
            };
        }
        let Some(node) = self.node else {
            return Ok((
                WorkflowPosition {
                    phase: 1,
                    item: self.window.admitted_count,
                },
                GeneralV2Action::BeginCandidate,
            ));
        };
        node.validate()
            .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
        if node.epoch != self.window.epoch
            || node.market != self.window.market
            || node.epoch_generation != self.epoch.generation
        {
            return Err(WorkflowGraphError::InvalidCanonicalState);
        }
        match node.status {
            AdmissionNodeStatusV1::Committed => Ok((
                WorkflowPosition {
                    phase: 2,
                    item: node.ordinal,
                },
                GeneralV2Action::WriteCandidateFeed,
            )),
            AdmissionNodeStatusV1::Revealed => {
                let Some((feed, sealed)) = self.feed else {
                    return Ok((
                        WorkflowPosition {
                            phase: 2,
                            item: node.ordinal,
                        },
                        GeneralV2Action::WriteCandidateFeed,
                    ));
                };
                feed.validate(sealed)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                if feed.epoch != node.epoch
                    || feed.node != node.node
                    || feed.market != node.market
                    || feed.epoch_generation != node.epoch_generation
                {
                    return Err(WorkflowGraphError::InvalidCanonicalState);
                }
                let complete = feed.prices_written == feed.outcome_count
                    && feed.fills_written == feed.order_count
                    && feed.atoms_written == feed.atom_count
                    && feed.slices_written == feed.slice_count;
                if !complete {
                    let item = u64::from(feed.prices_written)
                        + u64::from(feed.fills_written)
                        + u64::from(feed.atoms_written)
                        + u64::from(feed.slices_written);
                    return Ok((
                        WorkflowPosition { phase: 3, item },
                        GeneralV2Action::WriteCandidateFeed,
                    ));
                }
                if !sealed {
                    return Ok((
                        WorkflowPosition {
                            phase: 4,
                            item: node.ordinal,
                        },
                        GeneralV2Action::SealCandidate,
                    ));
                }
                let Some(work) = self.work else {
                    return Ok((
                        WorkflowPosition {
                            phase: 5,
                            item: node.ordinal,
                        },
                        GeneralV2Action::InitClearWork,
                    ));
                };
                work.validate()
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                if work.epoch != node.epoch
                    || work.node != node.node
                    || work.market != node.market
                    || work.epoch_generation != node.epoch_generation
                {
                    return Err(WorkflowGraphError::InvalidCanonicalState);
                }
                let (phase, item, action) = match work.phase {
                    0 => (6, 0, GeneralV2Action::GrowClearWork),
                    1 => (
                        7,
                        u64::from(work.order_cursor),
                        GeneralV2Action::AdvanceClearOrders,
                    ),
                    2 => (
                        8,
                        u64::from(work.slice_cursor),
                        GeneralV2Action::AdvanceClearSlices,
                    ),
                    3 => (
                        9,
                        node.ordinal,
                        GeneralV2Action::CompleteCandidateVerification,
                    ),
                    _ => return Err(WorkflowGraphError::InvalidCanonicalState),
                };
                Ok((WorkflowPosition { phase, item }, action))
            }
            AdmissionNodeStatusV1::VerifiedValid
            | AdmissionNodeStatusV1::VerifiedRefused
            | AdmissionNodeStatusV1::ExpiredCommitment
            | AdmissionNodeStatusV1::ExpiredUnverified => {
                let submissions_terminal = self
                    .window
                    .revealed_count
                    .checked_add(self.window.expired_commitment_count)
                    == Some(self.window.admitted_count);
                let verdicts_terminal = self
                    .window
                    .verdict_count
                    .checked_add(self.window.expired_unverified_count)
                    == Some(self.window.revealed_count);
                if self.window.finalized_slot == 0 && submissions_terminal && verdicts_terminal {
                    Ok((
                        WorkflowPosition {
                            phase: 9,
                            item: self.window.verdict_count,
                        },
                        GeneralV2Action::FinalizeSelection,
                    ))
                } else {
                    Err(WorkflowGraphError::NotReady)
                }
            }
        }
    }

    fn validate_payload(self, action: GeneralV2Action, payload: &[u8]) -> Result<()> {
        let epoch = self.window.epoch.bytes();
        let valid = match action {
            GeneralV2Action::BeginCandidate => {
                match decode_identity_lab_payload_v1(action.tag(), payload)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?
                {
                    IdentityLabPayloadV1::BeginCandidate(value) => value.epoch.bytes() == epoch,
                    _ => false,
                }
            }
            GeneralV2Action::WriteCandidateFeed => {
                let node = self.node.ok_or(WorkflowGraphError::ActionStateMismatch)?;
                match decode_identity_lab_payload_v1(action.tag(), payload)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?
                {
                    IdentityLabPayloadV1::WriteCandidateFeed(
                        WriteCandidateFeedPayloadV1::Open(value),
                    ) => value.epoch.bytes() == epoch && value.node == node.node,
                    IdentityLabPayloadV1::WriteCandidateFeed(
                        WriteCandidateFeedPayloadV1::Segment(value),
                    ) => value.epoch.bytes() == epoch && value.node == node.node,
                    _ => false,
                }
            }
            GeneralV2Action::SealCandidate
            | GeneralV2Action::InitClearWork
            | GeneralV2Action::CompleteCandidateVerification => {
                let node = self.node.ok_or(WorkflowGraphError::ActionStateMismatch)?;
                let decoded = decode_identity_lab_payload_v1(action.tag(), payload)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
                let value = match decoded {
                    IdentityLabPayloadV1::SealCandidate(value)
                    | IdentityLabPayloadV1::InitClearWork(value)
                    | IdentityLabPayloadV1::CompleteCandidateVerification(value) => value,
                    _ => return Err(WorkflowGraphError::ActionStateMismatch),
                };
                value.epoch.bytes() == epoch && value.node == node.node
            }
            GeneralV2Action::FinalizeSelection => {
                match decode_identity_lab_payload_v1(action.tag(), payload)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?
                {
                    IdentityLabPayloadV1::FinalizeSelection(value) => value.epoch.bytes() == epoch,
                    _ => false,
                }
            }
            GeneralV2Action::FreezeEntitlement => {
                match decode_owner_settlement_payload_v1(action.tag(), payload)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?
                {
                    OwnerSettlementPayloadV1::FreezeEntitlement(value) => {
                        value.epoch.bytes() == epoch
                            && value.selected_candidate == self.window.selected_candidate_artifact
                    }
                    _ => false,
                }
            }
            GeneralV2Action::GrowClearWork
            | GeneralV2Action::AdvanceClearOrders
            | GeneralV2Action::AdvanceClearSlices => true,
            _ => return Err(WorkflowGraphError::ActionStateMismatch),
        };
        if valid {
            Ok(())
        } else {
            Err(WorkflowGraphError::ActionStateMismatch)
        }
    }
}

pub fn plan_candidate_crank(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    observation: CandidateCrankObservation<'_>,
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    let generation = observation.epoch.generation;
    let (position, action) = observation.next()?;
    cursor.require(
        WorkflowLane::Candidate,
        generation,
        position,
        observation.observed_state_sha256,
    )?;
    observation.validate_payload(action, &material.payload)?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::GeneralV2Candidate,
        CanonicalActionCoordinate::General(action),
        material,
    )
}

/// Keeper state is derived through canonical owner-settlement pure transitions,
/// so the operator cannot invent a receipt transition identity.
#[derive(Clone, Copy, Debug)]
pub enum KeeperReceiptObservation {
    Direct(DirectEggSettlementInputV1),
    VirtualSplit(VirtualSplitReceiptInputV1),
    VirtualMerge(VirtualMergeReceiptInputV1),
    AccountEnd {
        owner: AuthenticatedOwnerSettlementAccountV1,
        receipt: AuthenticatedSettlementReceiptEndV1,
        epoch_generation: u64,
        joined_settlement_transition_id: [u8; 32],
    },
    FinalizeOwner {
        owner: AuthenticatedOwnerSettlementAccountV1,
        position: AuthenticatedPositionCashV1,
        fee: AuthenticatedOwnerFeeDebitV1,
        pot: SettlementCashPotV1,
        settlement_cash_pot_address: [u8; 32],
    },
}

impl KeeperReceiptObservation {
    fn next(self) -> Result<(u64, WorkflowPosition, GeneralV2Action)> {
        match self {
            Self::Direct(input) => {
                let _plan = prepare_direct_egg_settlement_v1(input)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                Ok((
                    input.buyer_position.generation,
                    WorkflowPosition {
                        phase: 1,
                        item: input.receipt.sequence,
                    },
                    GeneralV2Action::ConsumeDirectReceiptEggs,
                ))
            }
            Self::VirtualSplit(input) => {
                let _plan = prepare_virtual_split_receipt_v1(input)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                Ok((
                    input.position.generation,
                    WorkflowPosition {
                        phase: 2,
                        item: input.receipt.sequence,
                    },
                    GeneralV2Action::ConsumeVirtualSplitReceiptEggs,
                ))
            }
            Self::VirtualMerge(input) => {
                let _plan = prepare_virtual_merge_receipt_v1(input)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                Ok((
                    input.position.generation,
                    WorkflowPosition {
                        phase: 3,
                        item: input.receipt.sequence,
                    },
                    GeneralV2Action::ConsumeVirtualMergeReceiptEggs,
                ))
            }
            Self::AccountEnd {
                owner,
                receipt,
                epoch_generation,
                joined_settlement_transition_id,
            } => {
                let plan = prepare_account_receipt_end_v1(owner, receipt)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                if epoch_generation == 0
                    || joined_settlement_transition_id != plan.settlement_transition_id
                {
                    return Err(WorkflowGraphError::ActionStateMismatch);
                }
                Ok((
                    epoch_generation,
                    WorkflowPosition {
                        phase: 4,
                        item: receipt.sequence,
                    },
                    GeneralV2Action::AccountReceiptEnd,
                ))
            }
            Self::FinalizeOwner {
                owner,
                position,
                fee,
                pot,
                settlement_cash_pot_address,
            } => {
                let _plan = prepare_realize_owner_cash_v1(owner, position, fee, pot)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                if settlement_cash_pot_address == [0; 32] {
                    return Err(WorkflowGraphError::ZeroIdentity);
                }
                Ok((
                    position.generation,
                    WorkflowPosition {
                        phase: 5,
                        item: u64::from(owner.accumulator.consumed_slice_count),
                    },
                    GeneralV2Action::FinalizeOwnerSettlement,
                ))
            }
        }
    }

    fn validate_payload(self, payload: &[u8]) -> Result<()> {
        let valid = match self {
            Self::Direct(input) => {
                let value =
                    clutch_general_v2_contract::ConsumeDirectReceiptEggsPayloadV1::decode(payload)
                        .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
                value.epoch.bytes() == input.receipt.epoch
                    && value.receipt.bytes() == input.receipt.receipt
            }
            Self::VirtualSplit(input) => {
                let value =
                    clutch_general_v2_contract::ConsumeVirtualSplitReceiptEggsPayloadV1::decode(
                        payload,
                    )
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
                value.epoch.bytes() == input.receipt.epoch
                    && value.receipt.bytes() == input.receipt.receipt
            }
            Self::VirtualMerge(input) => {
                let value =
                    clutch_general_v2_contract::ConsumeVirtualMergeReceiptEggsPayloadV1::decode(
                        payload,
                    )
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
                value.epoch.bytes() == input.receipt.epoch
                    && value.receipt.bytes() == input.receipt.receipt
            }
            Self::AccountEnd { owner, receipt, .. } => {
                let value = clutch_general_v2_contract::AccountReceiptEndPayloadV1::decode(payload)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
                value.epoch.bytes() == receipt.epoch
                    && value.selected_candidate.bytes() == receipt.candidate
                    && value.owner_settlement.bytes() == owner.address
                    && value.receipt.bytes() == receipt.receipt
            }
            Self::FinalizeOwner {
                owner,
                position,
                pot,
                settlement_cash_pot_address,
                ..
            } => {
                let value =
                    clutch_general_v2_contract::FinalizeOwnerSettlementPayloadV1::decode(payload)
                        .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
                value.epoch.bytes() == owner.accumulator.expectation.epoch
                    && value.selected_candidate.bytes() == owner.accumulator.expectation.candidate
                    && value.owner_settlement.bytes() == owner.address
                    && value.position.bytes() == position.position
                    && value.settlement_cash_pot.bytes() == settlement_cash_pot_address
                    && pot.expectation.epoch == owner.accumulator.expectation.epoch
            }
        };
        if valid {
            Ok(())
        } else {
            Err(WorkflowGraphError::ActionStateMismatch)
        }
    }
}

pub fn plan_keeper_receipt(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    observation: KeeperReceiptObservation,
    observed_state_sha256: [u8; 32],
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    let (generation, position, action) = observation.next()?;
    cursor.require(
        WorkflowLane::KeeperReceipts,
        generation,
        position,
        observed_state_sha256,
    )?;
    observation.validate_payload(&material.payload)?;
    construct(
        manifest,
        builder,
        cursor,
        ProtocolFlow::KeeperSettlement,
        CanonicalActionCoordinate::General(action),
        material,
    )
}

/// Recovery and retirement actions whose canonical preconditions are available
/// to an unsigned operator without pretending that close authority is local.
#[derive(Clone, Copy, Debug)]
pub enum RecoveryObservation<'a> {
    ExpireCandidate(ExpireCommittedCandidateTransitionV1<'a>),
    CleanupCandidate(CleanupCandidateTransitionV1<'a>),
    ClaimSolver(ClaimSolverTransitionV1<'a>),
    CloseClearWork(CloseClearWorkTransitionV1<'a>),
    CloseSelected(&'a SelectedCandidateRetirementContractV1),
    CloseEpoch {
        epoch: &'a GeneralEpochV6AccountV1,
        window: &'a CandidateWindowV4AccountV1,
        budget: &'a EpochBudgetV2AccountV1,
        candidate_family: CandidateBundleRetirementContractV1,
    },
    ClosePosition(PositionAccountV3),
    CloseSeriesFunding {
        funding: &'a SeriesFundingAccountV1,
        funding_terms: &'a SeriesFundingTermsV2,
        quote: &'a SeriesFundingQuoteV1,
    },
    CloseSourceGeneration(&'a ReopenLineageV1),
    RetireStructuredClaim {
        descriptor: &'a StructuredClaimDescriptorV1,
        vault_generation: u64,
    },
}

impl RecoveryObservation<'_> {
    fn next(
        self,
    ) -> Result<(
        u64,
        WorkflowPosition,
        CanonicalActionCoordinate,
        ProtocolFlow,
    )> {
        match self {
            Self::ExpireCandidate(request) => {
                let _poststate = expire_committed_candidate_poststate_v1(request)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                Ok((
                    request.epoch.generation,
                    WorkflowPosition {
                        phase: 1,
                        item: request.node.ordinal,
                    },
                    CanonicalActionCoordinate::General(GeneralV2Action::ExpireCandidate),
                    ProtocolFlow::RecoveryRetirement,
                ))
            }
            Self::CleanupCandidate(request) => {
                let _poststate = cleanup_candidate_poststate_v1(request)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                Ok((
                    request.epoch.generation,
                    WorkflowPosition {
                        phase: 2,
                        item: request.node.ordinal,
                    },
                    CanonicalActionCoordinate::General(GeneralV2Action::CleanupCandidate),
                    ProtocolFlow::RecoveryRetirement,
                ))
            }
            Self::ClaimSolver(request) => {
                let _poststate = claim_solver_poststate_v1(request)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                Ok((
                    request.epoch.generation,
                    WorkflowPosition { phase: 3, item: 0 },
                    CanonicalActionCoordinate::General(GeneralV2Action::ClaimSolver),
                    ProtocolFlow::RecoveryRetirement,
                ))
            }
            Self::CloseClearWork(request) => {
                let _poststate = close_clear_work_poststate_v1(request)
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                Ok((
                    request.epoch.generation,
                    WorkflowPosition {
                        phase: 4,
                        item: u64::from(request.work.slice_cursor),
                    },
                    CanonicalActionCoordinate::General(GeneralV2Action::CloseClearWork),
                    ProtocolFlow::RecoveryRetirement,
                ))
            }
            Self::CloseSelected(contract) => {
                if !contract
                    .retirable()
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?
                {
                    return Err(WorkflowGraphError::NotReady);
                }
                Ok((
                    contract.epoch_generation,
                    WorkflowPosition {
                        phase: 5,
                        item: u64::from(contract.next_slice_index),
                    },
                    CanonicalActionCoordinate::General(GeneralV2Action::CloseCandidate),
                    ProtocolFlow::RecoveryRetirement,
                ))
            }
            Self::CloseEpoch {
                epoch,
                window,
                budget,
                candidate_family,
            } => {
                epoch
                    .validate()
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                let _window_disposition = window
                    .retirement_disposition()
                    .map_err(|_| WorkflowGraphError::NotReady)?;
                let _budget_disposition = budget
                    .retirement_disposition()
                    .map_err(|_| WorkflowGraphError::NotReady)?;
                if !candidate_family
                    .retired()
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?
                    || epoch.phase != GeneralEpochPhaseV1::Finalized
                    || epoch.candidate_bundle_count != 0
                    || epoch.work_count != 0
                    || epoch.selected_candidate_count != 0
                    || window.market != epoch.market_runtime
                    || window.epoch_generation != epoch.generation
                    || budget.market != epoch.market_runtime
                    || budget.epoch_generation != epoch.generation
                    || candidate_family.epoch_generation != epoch.generation
                {
                    return Err(WorkflowGraphError::NotReady);
                }
                Ok((
                    epoch.generation,
                    WorkflowPosition { phase: 7, item: 0 },
                    CanonicalActionCoordinate::General(GeneralV2Action::CloseEpoch),
                    ProtocolFlow::RecoveryRetirement,
                ))
            }
            Self::ClosePosition(position) => {
                position
                    .terminal_projection()
                    .map_err(|_| WorkflowGraphError::NotReady)?;
                Ok((
                    position.generation(),
                    WorkflowPosition { phase: 6, item: 0 },
                    CanonicalActionCoordinate::General(GeneralV2Action::ClosePosition),
                    ProtocolFlow::RecoveryRetirement,
                ))
            }
            Self::CloseSeriesFunding {
                funding,
                funding_terms,
                quote,
            } => {
                funding
                    .validate()
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                let _terminal_projection = funding
                    .state
                    .terminal_projection(funding_terms, quote)
                    .map_err(|_| WorkflowGraphError::NotReady)?;
                Ok((
                    1,
                    WorkflowPosition {
                        phase: 8,
                        item: u64::from(funding.state.next_ordinal),
                    },
                    CanonicalActionCoordinate::Series(RecurringSeriesAction::CloseFunding),
                    ProtocolFlow::ProductSeries,
                ))
            }
            Self::CloseSourceGeneration(lineage) => {
                lineage
                    .validate()
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                if !lineage.is_open || lineage.latest_generation == 0 {
                    return Err(WorkflowGraphError::NotReady);
                }
                Ok((
                    lineage.latest_generation,
                    WorkflowPosition {
                        phase: 9,
                        item: lineage.latest_generation,
                    },
                    CanonicalActionCoordinate::SourceRegistry(SourceSeriesAction::CloseGeneration),
                    ProtocolFlow::SourcePlaneV3,
                ))
            }
            Self::RetireStructuredClaim {
                descriptor,
                vault_generation,
            } => {
                descriptor
                    .validate_persisted()
                    .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
                if descriptor.state != DescriptorStateV1::Active || vault_generation == 0 {
                    return Err(WorkflowGraphError::InvalidCanonicalState);
                }
                Ok((
                    vault_generation,
                    WorkflowPosition { phase: 10, item: 0 },
                    CanonicalActionCoordinate::StructuredClaim(StructuredClaimActionV1::Retire),
                    ProtocolFlow::StructuredClaim,
                ))
            }
        }
    }
}

pub fn plan_recovery_or_retirement(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    observation: RecoveryObservation<'_>,
    observed_state_sha256: [u8; 32],
    cursor: ResumableWorkflowCursor,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    match observation {
        RecoveryObservation::ExpireCandidate(request) => {
            let decoded = decode_identity_lab_payload_v1(
                GeneralV2Action::ExpireCandidate.tag(),
                &material.payload,
            )
            .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
            match decoded {
                IdentityLabPayloadV1::ExpireCommittedCandidate(value)
                    if value == request.payload => {}
                _ => return Err(WorkflowGraphError::ActionStateMismatch),
            }
        }
        RecoveryObservation::CleanupCandidate(request) => {
            let decoded = decode_identity_lab_payload_v1(
                GeneralV2Action::CleanupCandidate.tag(),
                &material.payload,
            )
            .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
            match decoded {
                IdentityLabPayloadV1::CleanupCandidate(value) if value == request.payload => {}
                _ => return Err(WorkflowGraphError::ActionStateMismatch),
            }
        }
        RecoveryObservation::ClaimSolver(request) => {
            let decoded = decode_identity_lab_payload_v1(
                GeneralV2Action::ClaimSolver.tag(),
                &material.payload,
            )
            .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
            match decoded {
                IdentityLabPayloadV1::ClaimSolver(value) if value == request.payload => {}
                _ => return Err(WorkflowGraphError::ActionStateMismatch),
            }
        }
        RecoveryObservation::CloseClearWork(request) => {
            let decoded = decode_identity_lab_payload_v1(
                GeneralV2Action::CloseClearWork.tag(),
                &material.payload,
            )
            .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
            match decoded {
                IdentityLabPayloadV1::CloseClearWork(value) if value == request.payload => {}
                _ => return Err(WorkflowGraphError::ActionStateMismatch),
            }
        }
        RecoveryObservation::CloseSeriesFunding { funding, .. } => {
            let intent = CloseSeriesFundingIntentV1::decode(&material.payload)
                .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
            if intent.series_plan_id != funding.state.series_plan_id {
                return Err(WorkflowGraphError::ActionStateMismatch);
            }
        }
        RecoveryObservation::CloseSourceGeneration(lineage) => {
            if lineage.adapter_program.bytes() != manifest.clutch.program_id.to_bytes() {
                return Err(WorkflowGraphError::WrongProgramRelease);
            }
        }
        RecoveryObservation::RetireStructuredClaim {
            vault_generation, ..
        } => {
            let decoded = decode_structured_claim_payload_v1(
                StructuredClaimActionV1::Retire.tag(),
                &material.payload,
            )
            .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
            match decoded {
                clutch_structured_claim_runtime_contract::StructuredClaimPayloadV1::Retire(
                    intent,
                ) if intent.vault_generation == vault_generation => {}
                _ => return Err(WorkflowGraphError::ActionStateMismatch),
            }
        }
        _ => {}
    }
    let (generation, position, coordinate, flow) = observation.next()?;
    cursor.require(
        WorkflowLane::RecoveryRetirement,
        generation,
        position,
        observed_state_sha256,
    )?;
    construct(manifest, builder, cursor, flow, coordinate, material)
}

fn construct(
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    cursor: ResumableWorkflowCursor,
    flow: ProtocolFlow,
    coordinate: CanonicalActionCoordinate,
    material: WorkflowActionMaterial,
) -> Result<PlannedWorkflowNode> {
    manifest.admits_owner(&material.semantic_owner)?;
    if builder.clutch_program() != manifest.clutch.program_id
        || builder.clutch_release_sha256() != manifest.clutch.elf_sha256
    {
        return Err(WorkflowGraphError::WrongProgramRelease);
    }
    if material.action_name.trim().is_empty()
        || material.payload.is_empty()
        || material.exact_equations.is_empty()
    {
        return Err(WorkflowGraphError::InvalidCanonicalPayload);
    }
    let draft = match coordinate {
        CanonicalActionCoordinate::General(action) => {
            validate_general_payload(action, &material.payload)?;
            OwnedInstructionDraft::allocated_successor(
                flow,
                material.action_name,
                material.semantic_owner,
                manifest.clutch.program_id,
                material.accounts,
                material.required_signers,
                material.exact_equations,
                ExtensionAction::GeneralV2(action),
                &material.payload,
            )
        }
        CanonicalActionCoordinate::Series(action) => {
            validate_series_payload(action, &material.payload)?;
            OwnedInstructionDraft::allocated_successor(
                flow,
                material.action_name,
                material.semantic_owner,
                manifest.clutch.program_id,
                material.accounts,
                material.required_signers,
                material.exact_equations,
                ExtensionAction::RecurringSeries(action),
                &material.payload,
            )
        }
        CanonicalActionCoordinate::SourceRegistry(action) => {
            OwnedInstructionDraft::allocated_successor(
                flow,
                material.action_name,
                material.semantic_owner,
                manifest.clutch.program_id,
                material.accounts,
                material.required_signers,
                material.exact_equations,
                ExtensionAction::SourceV3(action),
                &material.payload,
            )
        }
        CanonicalActionCoordinate::StructuredClaim(action) => {
            let decoded = decode_structured_claim_payload_v1(action.tag(), &material.payload)
                .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
            let _ = decoded;
            OwnedInstructionDraft::semantic_reserved_successor(
                flow,
                material.action_name,
                material.semantic_owner,
                manifest.clutch.program_id,
                material.accounts,
                material.required_signers,
                material.exact_equations,
                clutch_solana_layout::registry::ExtensionFamily::StructuredClaim,
                action.tag(),
                &material.payload,
            )
        }
        CanonicalActionCoordinate::SourceTransition {
            registry,
            transition,
        } => {
            let intent = IntentPreimageV3::decode(&material.payload)
                .map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)?;
            if intent.action() != transition
                || intent.adapter_program_id().bytes()
                    != manifest.clutch.program_id.to_bytes()
                || material.payload.len() != INTENT_PREIMAGE_BYTES
            {
                return Err(WorkflowGraphError::ActionStateMismatch);
            }
            let expected_registry = source_registry_action(transition)?;
            if registry != expected_registry {
                return Err(WorkflowGraphError::ActionStateMismatch);
            }
            OwnedInstructionDraft::allocated_successor(
                ProtocolFlow::SourcePlaneV3,
                material.action_name,
                material.semantic_owner,
                manifest.clutch.program_id,
                material.accounts,
                material.required_signers,
                material.exact_equations,
                ExtensionAction::SourceV3(registry),
                &material.payload,
            )
        }
    }
    .map_err(|_| WorkflowGraphError::Construction)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| WorkflowGraphError::Construction)?;
    Ok(PlannedWorkflowNode {
        manifest_sha256: manifest.manifest_sha256,
        cursor,
        coordinate,
        unsigned_transaction,
        reload_authoritative_accounts: true,
    })
}

fn source_registry_action(transition: TransitionActionV3) -> Result<SourceSeriesAction> {
    match transition {
        TransitionActionV3::InitializeSourceHead => Ok(SourceSeriesAction::InitializeHead),
        TransitionActionV3::OpenRawPage => Ok(SourceSeriesAction::OpenRawPage),
        TransitionActionV3::AppendBoundary => Ok(SourceSeriesAction::IngestBoundaryBatch),
        TransitionActionV3::SealRawPage => Ok(SourceSeriesAction::SealRawPage),
        TransitionActionV3::CreateWindowWork => Ok(SourceSeriesAction::InitializeWindowWork),
        TransitionActionV3::FoldWindowPage => Ok(SourceSeriesAction::FoldWindowPages),
        TransitionActionV3::SealWindow => Ok(SourceSeriesAction::SealWindow),
        TransitionActionV3::WriteTerminalResult | TransitionActionV3::WriteDrawdownResult => {
            Ok(SourceSeriesAction::EvaluateStatistic)
        }
        TransitionActionV3::ActivateSeries
        | TransitionActionV3::CreateSeriesInstance
        | TransitionActionV3::LapseSeriesOrdinal
        | TransitionActionV3::AdvanceExistingInstance => {
            Err(WorkflowGraphError::ActionStateMismatch)
        }
    }
}

fn validate_general_payload(action: GeneralV2Action, payload: &[u8]) -> Result<()> {
    let decoded = match action {
        GeneralV2Action::AccountReceiptEnd | GeneralV2Action::FinalizeOwnerSettlement => {
            decode_owner_settlement_payload_v1(action.tag(), payload).map(|_| ())
        }
        GeneralV2Action::ConsumeDirectReceiptEggs => {
            decode_direct_settlement_payload_v1(action.tag(), payload).map(|_| ())
        }
        GeneralV2Action::ConsumeVirtualSplitReceiptEggs
        | GeneralV2Action::ConsumeVirtualMergeReceiptEggs => {
            decode_virtual_settlement_payload_v1(action.tag(), payload).map(|_| ())
        }
        GeneralV2Action::InitEpoch
        | GeneralV2Action::FreezeEpoch
        | GeneralV2Action::BeginCandidate
        | GeneralV2Action::WriteCandidateFeed
        | GeneralV2Action::SealCandidate
        | GeneralV2Action::InitClearWork
        | GeneralV2Action::CompleteCandidateVerification
        | GeneralV2Action::FinalizeSelection
        | GeneralV2Action::ExpireCandidate
        | GeneralV2Action::CleanupCandidate
        | GeneralV2Action::ClaimSolver
        | GeneralV2Action::CloseClearWork => {
            decode_identity_lab_payload_v1(action.tag(), payload).map(|_| ())
        }
        _ if payload.len() <= clutch_general_v2_contract::MAX_GENERAL_V2_ACTION_PAYLOAD_BYTES => {
            Ok(())
        }
        _ => Err(clutch_general_v2_contract::CodecError::WrongLength),
    };
    decoded.map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)
}

fn validate_series_payload(action: RecurringSeriesAction, payload: &[u8]) -> Result<()> {
    let valid = match action {
        RecurringSeriesAction::RegisterSeries
            if payload.len() == REGISTER_SERIES_PAYLOAD_BYTES_V1 =>
        {
            RegisterSeriesIntentV1::decode(payload).map(|_| ())
        }
        RecurringSeriesAction::ActivateFunding
            if payload.len() == ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1 =>
        {
            ActivateSeriesFundingIntentV1::decode(payload).map(|_| ())
        }
        RecurringSeriesAction::AdvanceOccurrence
            if payload.len() == ADVANCE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1 =>
        {
            AdvanceSeriesOccurrenceIntentV1::decode(payload).map(|_| ())
        }
        RecurringSeriesAction::LapseOccurrence
            if payload.len() == LAPSE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1 =>
        {
            LapseSeriesOccurrenceIntentV1::decode(payload).map(|_| ())
        }
        RecurringSeriesAction::ObserveDonation
            if payload.len() == OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1 =>
        {
            ObserveSeriesDonationIntentV1::decode(payload).map(|_| ())
        }
        RecurringSeriesAction::CloseFunding
            if payload.len() == CLOSE_SERIES_FUNDING_PAYLOAD_BYTES_V1 =>
        {
            CloseSeriesFundingIntentV1::decode(payload).map(|_| ())
        }
        _ => return Err(WorkflowGraphError::InvalidCanonicalPayload),
    };
    valid.map_err(|_| WorkflowGraphError::InvalidCanonicalPayload)
}

/// Canonical helper for callers that decoded an owner row before deciding
/// whether an action-38 plan may be requested.
pub fn owner_accounting_is_complete(owner: &OwnerSettlementAccumulatorV1) -> Result<bool> {
    owner
        .validate()
        .map_err(|_| WorkflowGraphError::InvalidCanonicalState)?;
    Ok(owner.state == 0
        && owner.consumed_slice_count == owner.expectation.expected_slice_count
        && owner.consumed_buy_price_units == owner.expectation.expected_buy_price_units
        && owner.consumed_sell_price_units == owner.expectation.expected_sell_price_units
        && owner.completed_buy_order_mask == owner.expectation.expected_buy_order_mask
        && owner.completed_sell_order_mask == owner.expectation.expected_sell_order_mask)
}
