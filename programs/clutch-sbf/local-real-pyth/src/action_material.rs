//! Opaque, release-authenticated action material for operator projections.
//!
//! Construction accepts typed semantic-owner state and account projections,
//! never browser JSON or caller-authored instruction bytes. The resulting
//! artifact remains unsigned and blockhash-free. It can make an operator
//! control inspectable, but it cannot sign, submit, or predict poststate.

use crate::rpc_index::{CanonicalIntentCoordinate, IndexedProgramRelease};
use crate::operatord::KeeperActionSelection;
use crate::transaction_builder::{
    IntegerUnit, ProtocolFlow, ProtocolTransactionBuilder, RuntimeAdmission,
    UnsignedProtocolTransaction,
};
use crate::workflow_graph::{
    plan_source_crank, CanonicalActionCoordinate, ExplicitOperatorReleaseManifest,
    PlannedWorkflowNode, ResumableWorkflowCursor,
    SourceCrankObservation, SourceWorkflowActionMaterial, WorkflowGraphError,
};
use clutch_solana_layout::registry::{
    AllocationStatus, ExtensionAction, SOURCE_SERIES_FAMILY_TAG, SOURCE_SERIES_FAMILY_VERSION,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::BTreeSet;

pub const CANONICAL_ACTION_MATERIAL_SCHEMA_V1: &str =
    "dragons-clutch/operator-canonical-action-material/v1";

pub type Result<T> = core::result::Result<T, CanonicalActionMaterialErrorV1>;

/// Fail-closed construction errors. None grants execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalActionMaterialErrorV1 {
    InvalidRelease,
    ReleaseMismatch,
    CoordinateDisabled,
    WrongSelection,
    InvalidFreshness,
    FeePayerMismatch,
    InvalidPlan,
}

impl core::fmt::Display for CanonicalActionMaterialErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRelease => "canonical action material has an invalid checked release",
            Self::ReleaseMismatch => {
                "canonical action material differs from the checked program release"
            }
            Self::CoordinateDisabled => {
                "canonical action coordinate is not enabled by the checked release"
            }
            Self::WrongSelection => {
                "canonical action material differs from the selected finalized cursor"
            }
            Self::InvalidFreshness => "canonical action validity boundary is invalid",
            Self::FeePayerMismatch => {
                "transaction fee payer differs from the semantic account-role payer"
            }
            Self::InvalidPlan => "semantic-owner transaction construction was noncanonical",
        })
    }
}

impl std::error::Error for CanonicalActionMaterialErrorV1 {}

/// Slot boundary derived from the same bounded finalized acquisition as the
/// action inputs. A future launcher must acquire a recent blockhash separately
/// and discard this material after `valid_before_slot`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionFreshnessBoundaryV1 {
    pub observed_slot: u64,
    pub valid_before_slot: u64,
    pub maximum_validity_slots: u64,
}

/// Exact ordered account role retained by an opaque typed constructor. The
/// label is selected inside that constructor from the semantic owner's enum;
/// no public caller can construct this role from a string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRoleV1 {
    label: &'static str,
    address: Address,
    writable: bool,
    signer: bool,
}

impl CanonicalAccountRoleV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.label
    }

    #[must_use]
    pub const fn address(self) -> Address {
        self.address
    }

    #[must_use]
    pub const fn writable(self) -> bool {
        self.writable
    }

    #[must_use]
    pub const fn signer(self) -> bool {
        self.signer
    }
}

impl ActionFreshnessBoundaryV1 {
    fn validate(self) -> Result<()> {
        let lifetime = self
            .valid_before_slot
            .checked_sub(self.observed_slot)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidFreshness)?;
        if self.observed_slot == 0
            || lifetime == 0
            || self.maximum_validity_slots == 0
            || lifetime > self.maximum_validity_slots
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidFreshness);
        }
        Ok(())
    }
}

/// Server-owned action artifact. Fields are intentionally private so a caller
/// cannot combine a valid release verdict with independently shaped accounts,
/// cursor, signer set, transaction bytes, or freshness claims.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalActionMaterialV1 {
    release_key: String,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    coordinate: CanonicalIntentCoordinate,
    driver_account: Address,
    cursor: ResumableWorkflowCursor,
    freshness: ActionFreshnessBoundaryV1,
    fee_payer: Address,
    account_roles: Vec<CanonicalAccountRoleV1>,
    planned: PlannedWorkflowNode,
    draft_id: [u8; 32],
}

impl CanonicalActionMaterialV1 {
    #[must_use]
    pub fn release_key(&self) -> &str {
        &self.release_key
    }

    #[must_use]
    pub const fn release_manifest_sha256(&self) -> [u8; 32] {
        self.release_manifest_sha256
    }

    #[must_use]
    pub const fn capability_profile_id(&self) -> [u8; 32] {
        self.capability_profile_id
    }

    #[must_use]
    pub const fn coordinate(&self) -> CanonicalIntentCoordinate {
        self.coordinate
    }

    #[must_use]
    pub const fn driver_account(&self) -> Address {
        self.driver_account
    }

    #[must_use]
    pub const fn cursor(&self) -> ResumableWorkflowCursor {
        self.cursor
    }

    #[must_use]
    pub const fn freshness(&self) -> ActionFreshnessBoundaryV1 {
        self.freshness
    }

    #[must_use]
    pub const fn fee_payer(&self) -> Address {
        self.fee_payer
    }

    #[must_use]
    pub fn account_roles(&self) -> &[CanonicalAccountRoleV1] {
        &self.account_roles
    }

    #[must_use]
    pub fn unsigned_transaction(&self) -> &UnsignedProtocolTransaction {
        &self.planned.unsigned_transaction
    }

    #[must_use]
    pub const fn draft_id(&self) -> [u8; 32] {
        self.draft_id
    }

    #[must_use]
    pub const fn reload_authoritative_accounts(&self) -> bool {
        self.planned.reload_authoritative_accounts
    }

    /// Exact release/cursor join required before exposing this material as a
    /// callable verdict. Any rescan that changes the cursor invalidates it.
    #[must_use]
    pub fn matches(
        &self,
        release: &IndexedProgramRelease,
        coordinate: CanonicalIntentCoordinate,
        selection: &KeeperActionSelection,
    ) -> bool {
        self.release_key == release.key()
            && self.release_manifest_sha256 == release.release_manifest_sha256
            && self.capability_profile_id == release.capability_profile_id
            && self.coordinate == coordinate
            && self.driver_account == selection.account
            && self.cursor == selection.cursor
            && selection.release_key == self.release_key
            && selection.effective_commitment == crate::rpc_index::RpcCommitment::Finalized
            && self.planned.reload_authoritative_accounts
            && !self
                .planned
                .unsigned_transaction
                .has_recent_blockhash
            && !self.planned.unsigned_transaction.signed
            && !self.planned.unsigned_transaction.submitted
    }
}

/// Construct one Source material artifact through the sole typed Source graph.
/// The caller supplies decoded semantic-owner values and physical identities;
/// it cannot supply instruction bytes, account metas, signer vectors, or the
/// final transaction.
#[allow(clippy::too_many_arguments)]
pub fn construct_source_action_material_v1(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    selection: &KeeperActionSelection,
    observation: SourceCrankObservation<'_>,
    freshness: ActionFreshnessBoundaryV1,
    material: SourceWorkflowActionMaterial,
) -> Result<CanonicalActionMaterialV1> {
    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    manifest
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    if release.program_id != manifest.clutch.program_id
        || release.program_data != manifest.clutch.program_data
        || release.deployment_slot != manifest.clutch.deployment_slot
        || release.elf_sha256 != manifest.clutch.elf_sha256
        || release.release_manifest_sha256 != manifest.manifest_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let coordinate = CanonicalIntentCoordinate {
        family_tag: SOURCE_SERIES_FAMILY_TAG,
        family_version: SOURCE_SERIES_FAMILY_VERSION,
        local_action: material.accounts.action().tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    if selection.release_key != release.key()
        || selection.effective_commitment != crate::rpc_index::RpcCommitment::Finalized
        || selection.action != source_selection_action(material.accounts.action())
        || material.action_name != selection.action
        || selection.cursor != observation_cursor(observation, selection.cursor)?
        || freshness.observed_slot < selection.account_slot
        || material.valid_before_slot != freshness.valid_before_slot
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    if builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || builder.payer() != material.accounts.payer_address()
    {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let source_account_roles = material
        .accounts
        .ordered_projection()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let account_roles = source_account_roles
        .iter()
        .map(|role| CanonicalAccountRoleV1 {
            label: source_role_label_v2(role.role),
            address: role.address,
            writable: role.writable,
            signer: role.signer,
        })
        .collect::<Vec<_>>();
    if !account_roles
        .iter()
        .any(|role| role.address == selection.account)
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    let planned = plan_source_crank(
        manifest,
        builder,
        observation,
        selection.cursor,
        material,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let planned_coordinate_matches = matches!(
        planned.coordinate,
        CanonicalActionCoordinate::SourceTransition { registry, .. }
            if registry.tag() == coordinate.local_action
    );
    if planned.manifest_sha256 != release.release_manifest_sha256
        || planned.cursor != selection.cursor
        || !planned_coordinate_matches
        || !planned.reload_authoritative_accounts
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    validate_unsigned_source_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let release_key = release.key();
    let draft_id = action_material_id(
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        selection.account,
        selection.cursor,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        driver_account: selection.account,
        cursor: selection.cursor,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

pub(crate) const fn source_selection_action(
    action: clutch_solana_layout::registry::SourceSeriesAction,
) -> &'static str {
    use clutch_solana_layout::registry::SourceSeriesAction as Action;
    match action {
        Action::RegisterRelease => "register-source-release",
        Action::InitializeHead => "initialize-source-head",
        Action::OpenRawPage => "open-raw-page",
        Action::IngestBoundaryBatch => "ingest-boundary",
        Action::SealRawPage => "seal-raw-page",
        Action::InitializeWindowWork => "initialize-window-work",
        Action::FoldWindowPages => "fold-window-pages",
        Action::SealWindow => "seal-window",
        Action::EvaluateStatistic => "evaluate-statistic",
        Action::EmitFailureHandoff => "emit-failure-handoff",
        Action::ReopenGeneration => "reopen-source-generation",
        Action::CloseGeneration => "close-source-generation",
    }
}

pub(crate) fn source_action_from_selection(
    selection: &str,
) -> Option<clutch_solana_layout::registry::SourceSeriesAction> {
    use clutch_solana_layout::registry::SourceSeriesAction as Action;
    match selection {
        "register-source-release" => Some(Action::RegisterRelease),
        "initialize-source-head" => Some(Action::InitializeHead),
        "open-raw-page" => Some(Action::OpenRawPage),
        "ingest-boundary" => Some(Action::IngestBoundaryBatch),
        "seal-raw-page" => Some(Action::SealRawPage),
        "initialize-window-work" => Some(Action::InitializeWindowWork),
        "fold-window-pages" => Some(Action::FoldWindowPages),
        "seal-window" => Some(Action::SealWindow),
        "evaluate-statistic" => Some(Action::EvaluateStatistic),
        "emit-failure-handoff" => Some(Action::EmitFailureHandoff),
        "reopen-source-generation" => Some(Action::ReopenGeneration),
        "close-source-generation" => Some(Action::CloseGeneration),
        _ => None,
    }
}

fn observation_cursor(
    observation: SourceCrankObservation<'_>,
    cursor: ResumableWorkflowCursor,
) -> Result<ResumableWorkflowCursor> {
    if cursor.generation != observation.generation
        || cursor.observed_state_sha256 != observation.observed_state_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    Ok(cursor)
}

fn validate_unsigned_source_plan(
    coordinate: CanonicalIntentCoordinate,
    fee_payer: Address,
    roles: &[CanonicalAccountRoleV1],
    planned: &PlannedWorkflowNode,
) -> Result<()> {
    let transaction = &planned.unsigned_transaction;
    let expected_signers = roles
        .iter()
        .filter(|role| role.signer)
        .map(|role| role.address)
        .chain(core::iter::once(fee_payer))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let binding_matches = matches!(
        transaction.registry_bindings.as_slice(),
        [Some(binding)]
            if binding.family.tag() == coordinate.family_tag
                && binding.family.version() == coordinate.family_version
                && binding.local_action == coordinate.local_action
                && binding.family_status == AllocationStatus::Frozen
                && matches!(
                    binding.central_action,
                    Some(ExtensionAction::SourceV3(action))
                        if action.tag() == coordinate.local_action
                )
    );
    if transaction.flows != [ProtocolFlow::SourcePlaneV3]
        || transaction.actions.len() != 1
        || transaction.semantic_owners.len() != 1
        || !binding_matches
        || transaction.runtime_admissions != [RuntimeAdmission::ReleaseBoundEnabled]
        || transaction.required_signers != expected_signers
        || transaction.exact_equations.is_empty()
        || transaction.serialized_transaction.is_empty()
        || transaction.has_recent_blockhash
        || transaction.signed
        || transaction.submitted
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn action_material_id(
    release_key: &str,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    coordinate: CanonicalIntentCoordinate,
    driver_account: Address,
    cursor: ResumableWorkflowCursor,
    freshness: ActionFreshnessBoundaryV1,
    fee_payer: Address,
    roles: &[CanonicalAccountRoleV1],
    transaction: &UnsignedProtocolTransaction,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(CANONICAL_ACTION_MATERIAL_SCHEMA_V1.as_bytes());
    hash_text(&mut hash, release_key);
    hash.update(release_manifest_sha256);
    hash.update(capability_profile_id);
    hash.update([
        coordinate.family_tag,
        coordinate.family_version,
        coordinate.local_action,
    ]);
    hash.update(driver_account.to_bytes());
    hash.update(cursor.workflow_id);
    hash.update([workflow_lane_byte(cursor.lane)]);
    hash.update(cursor.generation.to_le_bytes());
    hash.update(cursor.position.phase.to_le_bytes());
    hash.update(cursor.position.item.to_le_bytes());
    hash.update(cursor.observed_state_sha256);
    hash.update(freshness.observed_slot.to_le_bytes());
    hash.update(freshness.valid_before_slot.to_le_bytes());
    hash.update(freshness.maximum_validity_slots.to_le_bytes());
    hash.update(fee_payer.to_bytes());
    hash.update(
        u64::try_from(roles.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (index, role) in roles.iter().enumerate() {
        // The release-enabled action plus canonical contract index owns the
        // role identity; no unstable Rust enum discriminant enters the hash.
        hash.update(
            u64::try_from(index)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hash_text(&mut hash, role.label);
        hash.update(role.address.to_bytes());
        hash.update([u8::from(role.writable), u8::from(role.signer)]);
    }
    hash.update(
        u64::try_from(transaction.actions.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for action in &transaction.actions {
        hash_text(&mut hash, action);
    }
    for owner in &transaction.semantic_owners {
        hash_text(&mut hash, &owner.package);
        hash_text(&mut hash, &owner.schema);
        hash.update(owner.release_sha256);
    }
    for equation in &transaction.exact_equations {
        hash_text(&mut hash, &equation.name);
        hash_integer_unit(&mut hash, equation.unit);
        hash.update(equation.left.to_le_bytes());
        hash.update(equation.right.to_le_bytes());
    }
    hash.update(
        u64::try_from(transaction.serialized_transaction.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hash.update(&transaction.serialized_transaction);
    hash.finalize().into()
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update(
        u64::try_from(value.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hash.update(value.as_bytes());
}

fn hash_integer_unit(hash: &mut Sha256, unit: IntegerUnit) {
    match unit {
        IntegerUnit::Lamports => hash.update([0]),
        IntegerUnit::CollateralAtoms { mint } => {
            hash.update([1]);
            hash.update(mint.to_bytes());
        }
        IntegerUnit::PriceUnits { scale } => {
            hash.update([2]);
            hash.update(scale.to_le_bytes());
        }
        IntegerUnit::EggAtoms { market, outcome } => {
            hash.update([3]);
            hash.update(market);
            hash.update([outcome]);
        }
        IntegerUnit::FeeAtoms { mint } => {
            hash.update([4]);
            hash.update(mint.to_bytes());
        }
        IntegerUnit::WrapperAtoms { mint } => {
            hash.update([5]);
            hash.update(mint.to_bytes());
        }
    }
}

pub(crate) const fn source_role_label_v2(
    role: clutch_solana_layout::source_series::SourceAccountRoleV2,
) -> &'static str {
    use clutch_solana_layout::source_series::SourceAccountRoleV2 as Role;
    match role {
        Role::SourceReleaseArtifact => "source-release-artifact",
        Role::SourceRelease => "source-release",
        Role::AdapterProgram => "adapter-program",
        Role::AdapterProgramData => "adapter-program-data",
        Role::ParserProgram => "parser-program",
        Role::ParserProgramData => "parser-program-data",
        Role::ParserConfig => "parser-config",
        Role::SourceSpec => "source-spec",
        Role::SourceWorkSchedule => "source-work-schedule",
        Role::GenerationRequest => "generation-request",
        Role::ClockSysvar => "clock-sysvar",
        Role::Feed => "feed",
        Role::ReceiverProgram => "receiver-program",
        Role::ReceiverProgramData => "receiver-program-data",
        Role::ReceiverConfig => "receiver-config",
        Role::SourceHead => "source-head",
        Role::HeadLineage => "head-lineage",
        Role::OpenRawPage => "open-raw-page",
        Role::OpenPageLineage => "open-page-lineage",
        Role::RawPage => "raw-page",
        Role::SourceOccurrence => "source-occurrence",
        Role::WindowSpec => "window-spec",
        Role::WindowWork => "window-work",
        Role::WorkLineage => "work-lineage",
        Role::WindowSeal => "window-seal",
        Role::StatisticKey => "statistic-key",
        Role::SummaryProgram => "summary-program",
        Role::EvaluatorProgram => "evaluator-program",
        Role::EvaluatorProgramData => "evaluator-program-data",
        Role::StatisticResult => "statistic-result",
        Role::ResultLineage => "result-lineage",
        Role::SourceWorkReceipt => "source-work-receipt",
        Role::LivenessPolicy => "liveness-policy",
        Role::SourceCompartment => "source-compartment",
        Role::Keeper => "keeper",
        Role::Payer => "payer",
        Role::PrincipalRefund => "principal-refund",
        Role::NeutralSink => "neutral-sink",
        Role::FailurePolicy => "failure-policy",
        Role::HandoffReceipt => "handoff-receipt",
        Role::GenerationAuthority => "generation-authority",
        Role::GenerationTarget => "generation-target",
        Role::GenerationLineage => "generation-lineage",
        Role::SystemProgram => "system-program",
        Role::RentSysvar => "rent-sysvar",
    }
}

const fn workflow_lane_byte(lane: crate::workflow_graph::WorkflowLane) -> u8 {
    match lane {
        crate::workflow_graph::WorkflowLane::Creation => 0,
        crate::workflow_graph::WorkflowLane::SourceCrank => 1,
        crate::workflow_graph::WorkflowLane::Candidate => 2,
        crate::workflow_graph::WorkflowLane::KeeperReceipts => 3,
        crate::workflow_graph::WorkflowLane::RecoveryRetirement => 4,
    }
}

impl From<WorkflowGraphError> for CanonicalActionMaterialErrorV1 {
    fn from(_: WorkflowGraphError) -> Self {
        Self::InvalidPlan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction_builder::{ExactEquation, SemanticOwner, CONSTRUCTION_PLAN_SCHEMA};
    use crate::workflow_graph::{WorkflowLane, WorkflowPosition};
    use clutch_solana_layout::source_series::SourceAccountRoleV2;

    fn address(byte: u8) -> Address {
        Address::new_from_array([byte; 32])
    }

    fn cursor() -> ResumableWorkflowCursor {
        ResumableWorkflowCursor {
            workflow_id: [9; 32],
            lane: WorkflowLane::SourceCrank,
            generation: 3,
            position: WorkflowPosition { phase: 2, item: 4 },
            observed_state_sha256: [8; 32],
        }
    }

    fn transaction() -> UnsignedProtocolTransaction {
        UnsignedProtocolTransaction {
            schema: CONSTRUCTION_PLAN_SCHEMA,
            flows: vec![ProtocolFlow::SourcePlaneV3],
            actions: vec!["open-raw-page".into()],
            semantic_owners: vec![SemanticOwner {
                package: "clutch-source-plane-v3-adapter".into(),
                schema: "intent-preimage-v3".into(),
                release_sha256: [7; 32],
            }],
            registry_bindings: vec![None],
            runtime_admissions: vec![RuntimeAdmission::ReleaseBoundEnabled],
            required_signers: vec![address(6)],
            exact_equations: vec![ExactEquation {
                name: "exact ceiling".into(),
                unit: IntegerUnit::Lamports,
                left: 11,
                right: 11,
            }],
            serialized_transaction: vec![1, 2, 3],
            has_recent_blockhash: false,
            signed: false,
            submitted: false,
        }
    }

    #[test]
    fn validity_boundary_refuses_zero_or_unbounded_lifetime() {
        assert_eq!(
            ActionFreshnessBoundaryV1 {
                observed_slot: 10,
                valid_before_slot: 10,
                maximum_validity_slots: 4,
            }
            .validate(),
            Err(CanonicalActionMaterialErrorV1::InvalidFreshness)
        );
        assert_eq!(
            ActionFreshnessBoundaryV1 {
                observed_slot: 10,
                valid_before_slot: 15,
                maximum_validity_slots: 4,
            }
            .validate(),
            Err(CanonicalActionMaterialErrorV1::InvalidFreshness)
        );
    }

    #[test]
    fn material_identity_commits_freshness_and_exact_role_address() {
        let coordinate = CanonicalIntentCoordinate {
            family_tag: SOURCE_SERIES_FAMILY_TAG,
            family_version: SOURCE_SERIES_FAMILY_VERSION,
            local_action: 3,
        };
        let roles = [CanonicalAccountRoleV1 {
            label: source_role_label_v2(SourceAccountRoleV2::Payer),
            address: address(6),
            writable: true,
            signer: true,
        }];
        let first = action_material_id(
            "release",
            [1; 32],
            [2; 32],
            coordinate,
            address(3),
            cursor(),
            ActionFreshnessBoundaryV1 {
                observed_slot: 10,
                valid_before_slot: 12,
                maximum_validity_slots: 4,
            },
            address(6),
            &roles,
            &transaction(),
        );
        let mut rebound = roles;
        rebound[0].address = address(5);
        let second = action_material_id(
            "release",
            [1; 32],
            [2; 32],
            coordinate,
            address(3),
            cursor(),
            ActionFreshnessBoundaryV1 {
                observed_slot: 10,
                valid_before_slot: 13,
                maximum_validity_slots: 4,
            },
            address(6),
            &rebound,
            &transaction(),
        );
        assert_ne!(first, second);
    }
}
