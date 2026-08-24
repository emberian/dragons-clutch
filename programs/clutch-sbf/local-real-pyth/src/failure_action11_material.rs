//! Finalized-chain unsigned material for current Failure action 11.
//!
//! There is intentionally no JSON/manual request shape. The constructor owns
//! the exact 42-role account tuple and derives sequence, bounded progress, the
//! reward equation, privileges, and payload from current RootV3, LinkV2,
//! Failure, liveness, Product, and Source semantic-owner bodies.

use crate::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    RpcCommitment,
};
use crate::transaction_builder::{
    ConstructionError, ExactEquation, IntegerUnit, OwnedInstructionDraft,
    ProtocolTransactionBuilder, SemanticOwner, TransactionTransport,
    UnsignedProtocolTransaction,
};
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    FailureMarketIntervalCellPhaseV2, FailureMarketIntervalCellV2,
};
use clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryV2;
use clutch_failure_policy_runtime::market_policy_v1::FailureMarketAdmissionStateV1;
use clutch_failure_policy_runtime::market_runtime_v1::{
    FailureMarketRuntimePhaseV1, FailureMarketRuntimeV1,
};
use clutch_failure_policy_runtime::market_quote_v1::FailureMarketRecoveryQuoteScheduleV1;
use clutch_liveness::{
    RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1, RuntimeCompartmentV1,
    RuntimeLivenessPolicyV1,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV6, EvidenceOnlyRecoveryPolicyV1, FixedCodec,
    MarketGenesisProfileV2, MarketInstancePreimageV2, MarketLifecyclePhaseV2,
    NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4,
    RegistryCapabilityProfileV4, RegistryProgramReleaseV2, SeriesAttachmentPlanV5,
    RegistryReleaseLocusV2, SeriesFundingQuoteV5, SeriesMarketLinkPhaseV2, SeriesPlanV5,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::failure_market_interval_v2::{
    FailureMarketIntervalCellAccountV2, FailureMarketIntervalHistoryAccountV2,
};
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FailureMarketRootAccountV3,
    FailureMarketRuntimeRootAccountV1, FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1,
    FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1, FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
    FAILURE_LIVENESS_POLICY_BODY_BYTES_V1, FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3,
    FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2, SeriesMarketLinkAccountV2, SeriesRegistryAccountV3,
};
use clutch_solana_layout::registry::{self, ExtensionFamily, RecoveryAction};
use clutch_source_plane_v3::{
    FixedCodec as SourceFixedCodec, StatisticKeyV3, StatisticResultV3, SummaryProgramV3,
    WindowSealV3, WindowSpecV3,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    authenticate_source_release_account, authenticate_source_route, source_occurrence_record_id,
    ReopenLineageV1, RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey,
    SourcePolicyHandoffAccountV1, SourceReleaseManifestV2, SourceWorkReceiptAccountV1,
    SourceWorkScheduleBindingV1,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;

pub const FAILURE_ACTION11_VALIDITY_SLOTS_V1: u64 = 32;
pub const FAILURE_ACTION11_ACCOUNT_COUNT_V1: usize = 42;
pub const FAILURE_ACTION11_LOCAL_ACTION_V1: u8 = 11;

const OWNER_PACKAGE: &str =
    "clutch-failure-policy-runtime+clutch-product-series+clutch-source-plane-v3-runtime";
const OWNER_SCHEMA: &str = "dragons-clutch/operator/failure-action11-material/v1";
const SEED_PRODUCT_ARTIFACT: &[u8] = b"dc:product-artifact:v1";
const SEED_MARKET_ROOT: &[u8] = b"dc:market-lifecycle-root:v1";
const SEED_SERIES_LINK: &[u8] = b"dc:series-market-link:v1";
const SEED_FAILURE_ADMISSION: &[u8] = b"dc:failure-market-root:v2";
const SEED_FAILURE_RUNTIME: &[u8] = b"dc:failure-root:v2";
const SEED_FAILURE_CELL: &[u8] = b"dc:fail-int-cell:v2";
const SEED_FAILURE_HISTORY: &[u8] = b"dc:fail-int-history:v2";
const SEED_FAILURE_POLICY: &[u8] = b"dc:failure-live-policy:v1";
const SEED_FAILURE_RECOVERY: &[u8] = b"dc:failure-recovery:v1";
const SEED_SOURCE_WORK_SCHEDULE: &[u8] = b"dc:product-artifact:v1";

pub type FailureAction11MaterialResult<T> =
    core::result::Result<T, FailureAction11MaterialError>;
type Result<T> = FailureAction11MaterialResult<T>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureAction11MaterialError {
    CheckedRelease,
    ChainSnapshot,
    ChainAuthority,
    NoCanonicalProgress,
    Funding,
    Arithmetic,
    Construction,
    MissingProductTerminalPreauthorization,
}

impl core::fmt::Display for FailureAction11MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit current Failure action 11",
            Self::ChainSnapshot => "Failure action accounts are not one finalized snapshot",
            Self::ChainAuthority => "current Failure/Product/Source authority refused",
            Self::NoCanonicalProgress => "active Failure cell has no canonical progress step",
            Self::Funding => "Recovery custody cannot fund the exact next step",
            Self::Arithmetic => "Failure action-11 exact arithmetic overflowed",
            Self::Construction => "release-bound Failure action-11 construction refused",
            Self::MissingProductTerminalPreauthorization => {
                "current Product state has no exact Failure action-13 terminal preauthorization"
            }
        })
    }
}

impl std::error::Error for FailureAction11MaterialError {}

/// Every current action-11 role, in semantic names rather than numeric or
/// caller-supplied account vectors.
#[derive(Clone, Copy, Debug)]
pub struct FailureAction11ChainSnapshotV1<'a> {
    pub market_lifecycle_root: &'a ObservedRpcAccount,
    pub series_market_link: &'a ObservedRpcAccount,
    pub failure_admission_root: &'a ObservedRpcAccount,
    pub failure_runtime_root: &'a ObservedRpcAccount,
    pub failure_interval_cell: &'a ObservedRpcAccount,
    pub failure_interval_history: &'a ObservedRpcAccount,
    pub series_registry: &'a ObservedRpcAccount,
    pub registry_program: &'a ObservedRpcAccount,
    pub registry_program_data: &'a ObservedRpcAccount,
    pub registry_release_artifact: &'a ObservedRpcAccount,
    pub capability_profile_artifact: &'a ObservedRpcAccount,
    pub compiler_bundle_artifact: &'a ObservedRpcAccount,
    pub funding_quote_artifact: &'a ObservedRpcAccount,
    pub series_plan_artifact: &'a ObservedRpcAccount,
    pub product_template_artifact: &'a ObservedRpcAccount,
    pub native_claim_basis_artifact: &'a ObservedRpcAccount,
    pub recovery_policy_artifact: &'a ObservedRpcAccount,
    pub price_measure_policy_artifact: &'a ObservedRpcAccount,
    pub market_genesis_artifact: &'a ObservedRpcAccount,
    pub attachment_plan_artifact: &'a ObservedRpcAccount,
    pub market_instance_artifact: &'a ObservedRpcAccount,
    pub source_release: &'a ObservedRpcAccount,
    pub source_adapter_program: &'a ObservedRpcAccount,
    pub source_adapter_program_data: &'a ObservedRpcAccount,
    pub source_parser_program: &'a ObservedRpcAccount,
    pub source_parser_program_data: &'a ObservedRpcAccount,
    pub source_parser_config: &'a ObservedRpcAccount,
    pub source_spec: &'a ObservedRpcAccount,
    pub source_work_schedule: &'a ObservedRpcAccount,
    pub source_occurrence: &'a ObservedRpcAccount,
    pub source_window: &'a ObservedRpcAccount,
    pub source_statistic_key: &'a ObservedRpcAccount,
    pub source_summary: &'a ObservedRpcAccount,
    pub source_window_seal: &'a ObservedRpcAccount,
    pub source_statistic_result: &'a ObservedRpcAccount,
    pub source_result_lineage: &'a ObservedRpcAccount,
    pub source_handoff_receipt: &'a ObservedRpcAccount,
    pub source_work_receipt: &'a ObservedRpcAccount,
    pub failure_liveness_policy: &'a ObservedRpcAccount,
    pub failure_recovery_compartment: &'a ObservedRpcAccount,
    pub keeper: &'a ObservedRpcAccount,
    pub recovery_refund_owner: &'a ObservedRpcAccount,
}

impl<'a> FailureAction11ChainSnapshotV1<'a> {
    fn ordered(self) -> [&'a ObservedRpcAccount; FAILURE_ACTION11_ACCOUNT_COUNT_V1] {
        [
            self.market_lifecycle_root,
            self.series_market_link,
            self.failure_admission_root,
            self.failure_runtime_root,
            self.failure_interval_cell,
            self.failure_interval_history,
            self.series_registry,
            self.registry_program,
            self.registry_program_data,
            self.registry_release_artifact,
            self.capability_profile_artifact,
            self.compiler_bundle_artifact,
            self.funding_quote_artifact,
            self.series_plan_artifact,
            self.product_template_artifact,
            self.native_claim_basis_artifact,
            self.recovery_policy_artifact,
            self.price_measure_policy_artifact,
            self.market_genesis_artifact,
            self.attachment_plan_artifact,
            self.market_instance_artifact,
            self.source_release,
            self.source_adapter_program,
            self.source_adapter_program_data,
            self.source_parser_program,
            self.source_parser_program_data,
            self.source_parser_config,
            self.source_spec,
            self.source_work_schedule,
            self.source_occurrence,
            self.source_window,
            self.source_statistic_key,
            self.source_summary,
            self.source_window_seal,
            self.source_statistic_result,
            self.source_result_lineage,
            self.source_handoff_receipt,
            self.source_work_receipt,
            self.failure_liveness_policy,
            self.failure_recovery_compartment,
            self.keeper,
            self.recovery_refund_owner,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct ChainDerivedFailureAction11MaterialV1 {
    checked_release_key: String,
    program_id: Address,
    program_data: Address,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    observed_slot: u64,
    valid_before_slot: u64,
    generation: u64,
    transition_nonce: u64,
    sequence: u64,
    requested_coordinates: u16,
    exact_reward_lamports: u64,
    remaining_work_before_lamports: u64,
    remaining_work_after_lamports: u64,
    state_sha256: [u8; 32],
    ordered_accounts: Vec<AccountMeta>,
}

impl ChainDerivedFailureAction11MaterialV1 {
    pub const fn sequence(&self) -> u64 { self.sequence }
    pub const fn requested_coordinates(&self) -> u16 { self.requested_coordinates }
    pub const fn exact_reward_lamports(&self) -> u64 { self.exact_reward_lamports }
    pub const fn observed_slot(&self) -> u64 { self.observed_slot }
    pub const fn valid_before_slot(&self) -> u64 { self.valid_before_slot }
    pub const fn state_sha256(&self) -> [u8; 32] { self.state_sha256 }

    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn transition_nonce(&self) -> u64 { self.transition_nonce }
    pub(crate) fn account_metas(&self) -> &[AccountMeta] { &self.ordered_accounts }
    pub(crate) fn driver_account(&self) -> Address { self.ordered_accounts[4].pubkey }

    pub fn unsigned_instruction(
        &self,
        release: &IndexedProgramRelease,
    ) -> Result<OwnedInstructionDraft> {
        authenticate_material_release(self, release)?;
        OwnedInstructionDraft::checked_release_failure_action11_v1(
            release,
            SemanticOwner {
                package: OWNER_PACKAGE.into(),
                schema: OWNER_SCHEMA.into(),
                release_sha256: self.release_manifest_sha256,
            },
            self.ordered_accounts.clone(),
            vec![ExactEquation {
                name: "Recovery work principal funds the exact action-11 keeper reward".into(),
                unit: IntegerUnit::Lamports,
                left: u128::from(self.remaining_work_before_lamports),
                right: u128::from(self.remaining_work_after_lamports)
                    + u128::from(self.exact_reward_lamports),
            }],
            self.sequence,
            self.requested_coordinates,
        )
        .map_err(map_construction)
    }

    /// Compile the exact one-instruction, blockhash-free transaction. The fee
    /// payer must be disjoint from all action roles because action 11 has no
    /// signer role and Solana unions payer privileges globally.
    pub fn unsigned_transaction(
        &self,
        release: &IndexedProgramRelease,
        payer: Address,
        transport: TransactionTransport,
    ) -> Result<UnsignedProtocolTransaction> {
        let draft = self.unsigned_instruction(release)?;
        ProtocolTransactionBuilder::new(
            payer,
            release.program_id,
            release.release_manifest_sha256,
            transport,
        )
        .and_then(|builder| builder.build_atomic(core::slice::from_ref(&draft)))
        .map_err(map_construction)
    }
}

/// Derive the only currently callable Failure operator action from one exact
/// finalized snapshot. No action, sequence, coordinate count, account role,
/// privilege, or payload byte is accepted from the caller.
pub fn derive_failure_action11_material_v1(
    release: &IndexedProgramRelease,
    snapshot: FailureAction11ChainSnapshotV1<'_>,
) -> Result<ChainDerivedFailureAction11MaterialV1> {
    authenticate_release(release)?;
    let ordered = snapshot.ordered();
    authenticate_provenance(release, &ordered)?;
    authenticate_role_shapes(release, &ordered)?;

    let root_frame = MarketLifecycleRootAccountV2::decode(&snapshot.market_lifecycle_root.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let root = &root_frame.state;
    let root_binding = root.binding();
    if root.phase() != MarketLifecyclePhaseV2::Active
        || snapshot.market_lifecycle_root.lamports < root_frame.rent_principal_lamports
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.market_lifecycle_root.address,
        root_frame.stored_bump,
        &[
            SEED_MARKET_ROOT,
            &root_binding.market_instance_id.bytes(),
            &root_binding.generation.to_le_bytes(),
        ],
    )?;

    let link_frame = SeriesMarketLinkAccountV2::decode(&snapshot.series_market_link.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let link = link_frame.state;
    let link_binding = link.binding();
    if link.phase() != SeriesMarketLinkPhaseV2::Active
        || link_binding.market_instance_id != root_binding.market_instance_id
        || link_binding.generation != root_binding.generation
        || link_binding.market_root_account_id.bytes()
            != snapshot.market_lifecycle_root.address.to_bytes()
        || link_binding.market_binding_id
            != root_binding.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.series_market_link.address,
        link_frame.stored_bump,
        &[
            SEED_SERIES_LINK,
            &link_binding.series_plan_id.bytes(),
            &link_binding.ordinal.to_le_bytes(),
        ],
    )?;

    let root_bytes: &[u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3] = snapshot
        .failure_admission_root
        .data
        .as_slice()
        .try_into()
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let admission_frame = FailureMarketRootAccountV3::decode(root_bytes)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let admission = FailureMarketAdmissionStateV1::decode(&admission_frame.admission_body)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let policy = admission.binding().facts();
    let quote = FailureMarketRecoveryQuoteScheduleV1::decode(&admission_frame.recovery_quote_body)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    if policy.market_instance_id != root_binding.market_instance_id
        || policy.generation != root_binding.generation
        || policy.product_template_id.bytes() != root_binding.product_template_id.bytes()
        || policy.native_claim_basis_id.bytes() != root_binding.native_claim_basis_id.bytes()
        || policy.recovery_policy_id.bytes() != root_binding.recovery_policy_id.bytes()
        || policy.price_measure_policy_id.bytes() != root_binding.price_measure_policy_id.bytes()
        || policy.market_genesis_profile_id.bytes()
            != root_binding.market_genesis_profile_id.bytes()
        || policy.capability_profile_id.bytes() != root_binding.capability_profile_id.bytes()
        || quote.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?.bytes()
            != policy.recovery_quote_schedule_id.bytes()
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.failure_admission_root.address,
        admission_frame.bump,
        &[
            SEED_FAILURE_ADMISSION,
            &policy.market_instance_id.bytes(),
            &policy.generation.to_le_bytes(),
        ],
    )?;

    let runtime_bytes: &[u8; FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1] = snapshot
        .failure_runtime_root
        .data
        .as_slice()
        .try_into()
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let runtime_frame = FailureMarketRuntimeRootAccountV1::decode(runtime_bytes)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let runtime = FailureMarketRuntimeV1::decode_for_admission(&runtime_frame.runtime_body, admission)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    if runtime.phase() != FailureMarketRuntimePhaseV1::IntervalActive
        || runtime.runtime_account_id().bytes() != snapshot.failure_runtime_root.address.to_bytes()
        || snapshot.failure_runtime_root.lamports < runtime.root_funding().observed_balance_lamports
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.failure_runtime_root.address,
        runtime_frame.bump,
        &[
            SEED_FAILURE_RUNTIME,
            &policy.market_instance_id.bytes(),
            &policy.generation.to_le_bytes(),
        ],
    )?;

    let cell_bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = snapshot
        .failure_interval_cell
        .data
        .as_slice()
        .try_into()
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let cell_frame = FailureMarketIntervalCellAccountV2::decode(cell_bytes)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let cell = FailureMarketIntervalCellV2::decode_canonical(cell_frame.semantic_body())
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    if cell.phase() != FailureMarketIntervalCellPhaseV2::Active
        || cell.failure_policy_binding_id() != admission.binding().id()
        || cell.market_instance_id() != policy.market_instance_id
        || cell.generation() != policy.generation
        || cell.history_account().bytes() != snapshot.failure_interval_history.address.to_bytes()
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.failure_interval_cell.address,
        cell_frame.bump(),
        &[
            SEED_FAILURE_CELL,
            &policy.market_instance_id.bytes(),
            &policy.generation.to_le_bytes(),
        ],
    )?;
    let history_bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] = snapshot
        .failure_interval_history
        .data
        .as_slice()
        .try_into()
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let history_frame = FailureMarketIntervalHistoryAccountV2::decode(history_bytes)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let history = FailureMarketIntervalHistoryV2::decode_canonical(history_frame.semantic_body())
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    if history.failure_policy_binding_id() != cell.failure_policy_binding_id()
        || history.market_instance_id() != cell.market_instance_id()
        || history.generation() != cell.generation()
        || history.funding_receipt_id() != cell.funding_receipt_id()
        || history.completed_session_count() != cell.completed_session_count()
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.failure_interval_history.address,
        history_frame.bump(),
        &[
            SEED_FAILURE_HISTORY,
            &policy.market_instance_id.bytes(),
            &policy.generation.to_le_bytes(),
        ],
    )?;

    authenticate_registry_and_product(release, snapshot, root_binding, link_binding)?;
    authenticate_source(release, snapshot, policy, root_binding, link_binding, cell)?;
    let recovery = authenticate_recovery(release, snapshot, policy, quote)?;

    let work = cell
        .product_work()
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?
        .ok_or(FailureAction11MaterialError::NoCanonicalProgress)?;
    if work.market_instance_id() != policy.market_instance_id
        || work.maximum_coordinates_per_advance() != policy.maximum_coordinates_per_advance
        || work.maximum_coordinates_per_advance() != root_binding.maximum_coordinates_per_advance
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    let remaining_coordinates = work
        .total_coordinates()
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?
        .checked_sub(work.checked_coordinates())
        .ok_or(FailureAction11MaterialError::Arithmetic)?;
    let attempt = quote
        .attempts
        .get(usize::from(cell.attempt_index()))
        .ok_or(FailureAction11MaterialError::ChainAuthority)?;
    let attempt_remaining = attempt
        .max_progress_units
        .checked_sub(cell.accepted_progress_units())
        .ok_or(FailureAction11MaterialError::Arithmetic)?;
    let requested_u64 = remaining_coordinates
        .min(attempt_remaining)
        .min(quote.maximum_progress_units_per_call)
        .min(u64::from(work.maximum_coordinates_per_advance()))
        .min(u64::from(u16::MAX));
    let requested_coordinates = u16::try_from(requested_u64)
        .map_err(|_| FailureAction11MaterialError::Arithmetic)?;
    if requested_coordinates == 0 {
        return Err(FailureAction11MaterialError::NoCanonicalProgress);
    }
    let accepted_after = cell
        .accepted_progress_units()
        .checked_add(u64::from(requested_coordinates))
        .ok_or(FailureAction11MaterialError::Arithmetic)?;
    let exact_reward_lamports = quote
        .exact_progress_reward_lamports(
            cell.attempt_index(),
            cell.accepted_progress_units(),
            accepted_after,
        )
        .map_err(|_| FailureAction11MaterialError::Arithmetic)?;
    let remaining_work_after_lamports = recovery
        .remaining_work_lamports
        .checked_sub(exact_reward_lamports)
        .ok_or(FailureAction11MaterialError::Funding)?;
    if recovery.remaining_calls == 0 || exact_reward_lamports == 0 {
        return Err(FailureAction11MaterialError::Funding);
    }
    let sequence = runtime
        .transition_sequence()
        .checked_add(1)
        .ok_or(FailureAction11MaterialError::Arithmetic)?;
    let valid_before_slot = snapshot
        .failure_admission_root
        .provenance
        .slot
        .checked_add(FAILURE_ACTION11_VALIDITY_SLOTS_V1)
        .ok_or(FailureAction11MaterialError::Arithmetic)?;
    let ordered_accounts = ordered_action11_metas(&ordered)?;
    let state_sha256 = snapshot_digest(&ordered);

    Ok(ChainDerivedFailureAction11MaterialV1 {
        checked_release_key: release.key(),
        program_id: release.program_id,
        program_data: release.program_data,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        observed_slot: snapshot.failure_admission_root.provenance.slot,
        valid_before_slot,
        generation: policy.generation,
        transition_nonce: cell.transition_nonce(),
        sequence,
        requested_coordinates,
        exact_reward_lamports,
        remaining_work_before_lamports: recovery.remaining_work_lamports,
        remaining_work_after_lamports,
        state_sha256,
        ordered_accounts,
    })
}

/// Current action 13 remains nonconstructible until Product persists the exact
/// terminal preauthorization consumed by the Failure owner.
pub fn derive_failure_action13_material_v1() -> Result<OwnedInstructionDraft> {
    Err(FailureAction11MaterialError::MissingProductTerminalPreauthorization)
}

fn authenticate_release(release: &IndexedProgramRelease) -> Result<()> {
    release.validate().map_err(|_| FailureAction11MaterialError::CheckedRelease)?;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: ExtensionFamily::Recovery.tag(),
        family_version: ExtensionFamily::Recovery.version(),
        local_action: RecoveryAction::AdvanceIntervalConsensus.tag(),
    };
    if !release.families.contains(&CanonicalFamily::Failure)
        || release.enabled_intents.binary_search(&coordinate).is_err()
    {
        return Err(FailureAction11MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_material_release(
    material: &ChainDerivedFailureAction11MaterialV1,
    release: &IndexedProgramRelease,
) -> Result<()> {
    authenticate_release(release)?;
    if release.key() != material.checked_release_key
        || release.program_id != material.program_id
        || release.program_data != material.program_data
        || release.release_manifest_sha256 != material.release_manifest_sha256
        || release.capability_profile_id != material.capability_profile_id
    {
        return Err(FailureAction11MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_provenance(
    release: &IndexedProgramRelease,
    accounts: &[&ObservedRpcAccount; FAILURE_ACTION11_ACCOUNT_COUNT_V1],
) -> Result<()> {
    let first = &accounts[0].provenance;
    let release_key = release.key();
    if first.commitment != RpcCommitment::Finalized
        || first.slot == 0
        || first.release_key.as_str() != release_key.as_str()
        || accounts.iter().any(|account| {
            account.provenance.commitment != RpcCommitment::Finalized
                || account.provenance.slot != first.slot
                || account.provenance.cluster_key != first.cluster_key
                || account.provenance.release_key.as_str() != release_key.as_str()
        })
    {
        return Err(FailureAction11MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn authenticate_role_shapes(
    release: &IndexedProgramRelease,
    accounts: &[&ObservedRpcAccount; FAILURE_ACTION11_ACCOUNT_COUNT_V1],
) -> Result<()> {
    const EXECUTABLE: [usize; 3] = [7, 22, 24];
    for (index, account) in accounts.iter().enumerate() {
        if account.address == Address::default()
            || account.executable != EXECUTABLE.contains(&index)
        {
            return Err(FailureAction11MaterialError::ChainAuthority);
        }
        let mut other = index + 1;
        while other < accounts.len() {
            if account.address == accounts[other].address && !(index == 40 && other == 41) {
                return Err(FailureAction11MaterialError::ChainAuthority);
            }
            other += 1;
        }
    }
    for index in (0..=6).chain(9..=21).chain(28..=39) {
        if accounts[index].owner != release.program_id {
            return Err(FailureAction11MaterialError::ChainAuthority);
        }
    }
    Ok(())
}

fn authenticate_registry_and_product(
    release: &IndexedProgramRelease,
    snapshot: FailureAction11ChainSnapshotV1<'_>,
    root: clutch_product_series::MarketLifecycleBindingV2,
    link: clutch_product_series::SeriesMarketLinkBindingV2,
) -> Result<()> {
    let registry = SeriesRegistryAccountV3::decode(&snapshot.series_registry.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    if registry.series_plan_id != link.series_plan_id
        || registry.registry_release_id != root.registry_release_id
        || registry.capability_profile_id != root.capability_profile_id
        || registry.compiler_bundle_id != link.compiler_bundle_id
        || !registry.activation_consumed
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }

    macro_rules! artifact {
        ($account:expr, $kind:ident, $ty:ty, $expected:expr) => {{
            let expected = $expected;
            let value = <$ty>::decode(&$account.data)
                .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
            let actual = value.id()
                .map_err(|_| FailureAction11MaterialError::ChainAuthority)?
                .content_id();
            let (address, _) = Address::find_program_address(
                &[SEED_PRODUCT_ARTIFACT, &[ArtifactKind::$kind.byte()], &expected.bytes()],
                &release.program_id,
            );
            if actual != expected || $account.address != address {
                return Err(FailureAction11MaterialError::ChainAuthority);
            }
            value
        }};
    }
    let registry_release = artifact!(snapshot.registry_release_artifact, RegistryProgramReleaseV2, RegistryProgramReleaseV2, root.registry_release_id);
    let profile = artifact!(snapshot.capability_profile_artifact, RegistryCapabilityProfileV4, RegistryCapabilityProfileV4, root.capability_profile_id);
    let _bundle = artifact!(snapshot.compiler_bundle_artifact, CompiledProductSeriesBundleV6, CompiledProductSeriesBundleV6, link.compiler_bundle_id.content_id());
    let _quote = artifact!(snapshot.funding_quote_artifact, SeriesFundingQuoteV5, SeriesFundingQuoteV5, link.funding_quote_id.content_id());
    let _plan = artifact!(snapshot.series_plan_artifact, SeriesPlanV5, SeriesPlanV5, link.series_plan_id.content_id());
    let _template = artifact!(snapshot.product_template_artifact, ProductTemplateV4, ProductTemplateV4, root.product_template_id);
    let _basis = artifact!(snapshot.native_claim_basis_artifact, NativeClaimBasisV1, NativeClaimBasisV1, root.native_claim_basis_id);
    let _recovery = artifact!(snapshot.recovery_policy_artifact, EvidenceOnlyRecoveryPolicyV1, EvidenceOnlyRecoveryPolicyV1, root.recovery_policy_id);
    let _price = artifact!(snapshot.price_measure_policy_artifact, PriceMeasurePolicyV1, PriceMeasurePolicyV1, root.price_measure_policy_id);
    let _genesis = artifact!(snapshot.market_genesis_artifact, MarketGenesisProfileV2, MarketGenesisProfileV2, root.market_genesis_profile_id);
    let _attachment = artifact!(snapshot.attachment_plan_artifact, SeriesAttachmentPlanV5, SeriesAttachmentPlanV5, link.attachment_plan_id.content_id());
    let _market = artifact!(snapshot.market_instance_artifact, MarketInstancePreimageV2, MarketInstancePreimageV2, root.market_instance_id.content_id());
    if registry_release.program.bytes() != snapshot.registry_program.address.to_bytes()
        || registry_release.programdata.bytes()
            != snapshot.registry_program_data.address.to_bytes()
        || profile.rules.registry_release_id.bytes() != root.registry_release_id.bytes()
        || profile.rules.maximum_coordinates_per_advance != root.maximum_coordinates_per_advance
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    authenticate_loader_pair(
        snapshot.registry_program,
        snapshot.registry_program_data,
        registry_release,
    )?;
    Ok(())
}

fn authenticate_loader_pair(
    program: &ObservedRpcAccount,
    program_data: &ObservedRpcAccount,
    release: RegistryProgramReleaseV2,
) -> Result<()> {
    const PROGRAM_METADATA_BYTES: usize = 36;
    const PROGRAMDATA_METADATA_BYTES: usize = 45;
    let program_data_sha256: [u8; 32] = Sha256::digest(&program_data.data).into();
    if program.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || program_data.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || !program.executable
        || program_data.executable
        || program.data.len() < PROGRAM_METADATA_BYTES
        || program_data.data.len() < PROGRAMDATA_METADATA_BYTES
        || program.data.get(..4) != Some(2_u32.to_le_bytes().as_slice())
        || program_data.data.get(..4) != Some(3_u32.to_le_bytes().as_slice())
        || program.data.get(4..36) != Some(program_data.address.to_bytes().as_slice())
        || release.program.bytes() != program.address.to_bytes()
        || release.programdata.bytes() != program_data.address.to_bytes()
        || release.programdata_sha256.bytes() != program_data_sha256
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    let deployment_slot = u64::from_le_bytes(
        program_data.data[4..12]
            .try_into()
            .map_err(|_| FailureAction11MaterialError::ChainAuthority)?,
    );
    let upgrade_authority_is_canonical = match program_data.data[12] {
        0 => program_data.data[13..45].iter().all(|byte| *byte == 0),
        1 => program_data.data[13..45].iter().any(|byte| *byte != 0),
        _ => false,
    };
    if !upgrade_authority_is_canonical
        || release.deployment_slot != deployment_slot
        || match release.locus {
            RegistryReleaseLocusV2::SynthesizedGenesisZero => deployment_slot != 0,
            RegistryReleaseLocusV2::ObservedPositive => deployment_slot == 0,
        }
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    Ok(())
}

fn authenticate_source(
    release: &IndexedProgramRelease,
    snapshot: FailureAction11ChainSnapshotV1<'_>,
    policy: clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
    root: clutch_product_series::MarketLifecycleBindingV2,
    link: clutch_product_series::SeriesMarketLinkBindingV2,
    cell: FailureMarketIntervalCellV2,
) -> Result<()> {
    let manifest = SourceReleaseManifestV2::decode(&snapshot.source_release.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let recipe = PdaRecipeV3::source_release(
        manifest.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?,
    ).map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let derived = derive_recipe(release.program_id, recipe)?;
    let authenticated_release = authenticate_source_release_account(
        runtime_key(release.program_id),
        account_view(snapshot.source_release),
        derived,
    ).map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let route = authenticate_source_route(
        authenticated_release,
        account_view(snapshot.source_adapter_program),
        account_view(snapshot.source_adapter_program_data),
        account_view(snapshot.source_parser_program),
        account_view(snapshot.source_parser_program_data),
        account_view(snapshot.source_parser_config),
        account_view(snapshot.source_spec),
    ).map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    if manifest.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?.bytes()
            != policy.source_release_manifest_id.bytes()
        || snapshot.source_release.address.to_bytes() != policy.source_release_account_id.bytes()
        || route.release_authentication_id().bytes() != policy.source_release_authentication_id.bytes()
        || route.route_id().bytes() != root.source_route_id.bytes()
        || route.route_id().bytes() != link.source_route_id.bytes()
        || route.source_spec_id().bytes() != policy.source_spec_id.bytes()
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    let schedule = SourceWorkScheduleBindingV1::decode(&snapshot.source_work_schedule.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let schedule_id = schedule.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let (schedule_address, _) = Address::find_program_address(
        &[
            SEED_SOURCE_WORK_SCHEDULE,
            &[ArtifactKind::SourceWorkScheduleV1.byte()],
            &schedule_id.bytes(),
            &schedule_id.bytes(),
        ],
        &release.program_id,
    );
    if snapshot.source_work_schedule.address != schedule_address
        || schedule_id.bytes() != cell.session_schedule_id().bytes()
        || schedule_id != route.source_work_schedule_id()
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }

    let occurrence_id = source_occurrence_record_id(&snapshot.source_occurrence.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let window = WindowSpecV3::decode(&snapshot.source_window.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let key = StatisticKeyV3::decode(&snapshot.source_statistic_key.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let summary = SummaryProgramV3::decode(&snapshot.source_summary.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let _seal = WindowSealV3::decode(&snapshot.source_window_seal.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let _result = StatisticResultV3::decode(&snapshot.source_statistic_result.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let _lineage = ReopenLineageV1::decode(&snapshot.source_result_lineage.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    if occurrence_id.bytes() != link.source_occurrence_id.bytes()
        || snapshot.source_occurrence.address.to_bytes()
            != link.source_occurrence_account_id.bytes()
        || window.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?.bytes()
            != policy.primary_window_id.bytes()
        || key.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?.bytes()
            != policy.statistic_key_id.bytes()
        || summary.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?.bytes()
            != policy.summary_program_id.bytes()
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    let handoff = SourcePolicyHandoffAccountV1::decode(&snapshot.source_handoff_receipt.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let work_receipt = SourceWorkReceiptAccountV1::decode(&snapshot.source_work_receipt.data)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    if handoff.handoff_id().bytes() != cell.source_handoff_id().bytes()
        || handoff.release_authentication_id() != route.release_authentication_id()
        || handoff.route_id() != route.route_id()
        || handoff.occurrence_account().bytes() != snapshot.source_occurrence.address.to_bytes()
        || handoff.result_account().bytes() != snapshot.source_statistic_result.address.to_bytes()
        || handoff.work_receipt_account().bytes() != snapshot.source_work_receipt.address.to_bytes()
        || handoff.failure_policy_binding_id().bytes() != admission_binding_bytes(policy, cell)
        || handoff.source_spec_id() != route.source_spec_id()
        || handoff.window_id().bytes() != policy.primary_window_id.bytes()
        || handoff.statistic_key_id().bytes() != policy.statistic_key_id.bytes()
        || handoff.generation() != link.source_repair_generation
        || work_receipt.receipt_account_id().bytes() != snapshot.source_work_receipt.address.to_bytes()
        || work_receipt.receipt_account_owner_program_id().bytes()
            != snapshot.source_work_receipt.owner.to_bytes()
        || work_receipt.route_id() != route.route_id()
        || work_receipt.source_work_schedule_id() != schedule_id
        || work_receipt.generation() != link.source_repair_generation
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    Ok(())
}

fn admission_binding_bytes(
    _policy: clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
    cell: FailureMarketIntervalCellV2,
) -> [u8; 32] {
    cell.failure_policy_binding_id().bytes()
}

fn authenticate_recovery(
    release: &IndexedProgramRelease,
    snapshot: FailureAction11ChainSnapshotV1<'_>,
    policy: clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
    quote: FailureMarketRecoveryQuoteScheduleV1,
) -> Result<RuntimeCompartmentV1> {
    let policy_frame = decode_failure_account_body_v1(
        &snapshot.failure_liveness_policy.data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    ).map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let liveness_policy = RuntimeLivenessPolicyV1::decode(policy_frame.body)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let recovery_frame = decode_failure_account_body_v1(
        &snapshot.failure_recovery_compartment.data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    ).map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let recovery = RuntimeCompartmentV1::decode(recovery_frame.body)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    recovery.validate_against_policy(liveness_policy)
        .map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let (policy_address, _) = Address::find_program_address(
        &[SEED_FAILURE_POLICY, &policy.liveness_policy_id.bytes()],
        &release.program_id,
    );
    let (recovery_address, _) = Address::find_program_address(
        &[
            SEED_FAILURE_RECOVERY,
            &policy.liveness_lifecycle_id.bytes(),
            &policy.generation.to_le_bytes(),
        ],
        &release.program_id,
    );
    if snapshot.failure_liveness_policy.data.len() != FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1
        || snapshot.failure_recovery_compartment.data.len()
            != FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1
        || snapshot.failure_liveness_policy.address != policy_address
        || snapshot.failure_recovery_compartment.address != recovery_address
        || recovery.kind != RuntimeCompartmentKindV1::Recovery
        || recovery.phase != RuntimeCompartmentPhaseV1::Active
        || recovery.identity.policy_id.bytes() != policy.liveness_policy_id.bytes()
        || recovery.identity.lifecycle_id.bytes() != policy.liveness_lifecycle_id.bytes()
        || recovery.identity.account_id.bytes()
            != snapshot.failure_recovery_compartment.address.to_bytes()
        || recovery.identity.owner.bytes() != release.program_id.to_bytes()
        || recovery.identity.payer.bytes() != snapshot.recovery_refund_owner.address.to_bytes()
        || recovery.identity.payer.bytes() != policy.recovery_refund_owner.bytes()
        || recovery.identity.generation != policy.generation
        || recovery.quote_schedule_id.bytes() != policy.recovery_quote_schedule_id.bytes()
        || recovery.maximum_calls != quote.maximum_calls
        || recovery.maximum_lamports_per_call
            != quote.maximum_lamports_per_call().map_err(|_| FailureAction11MaterialError::ChainAuthority)?
        || recovery.capitalized_work_lamports
            != quote.work_principal_lamports().map_err(|_| FailureAction11MaterialError::ChainAuthority)?
        || snapshot.failure_recovery_compartment.lamports
            < recovery.expected_account_balance_lamports()
                .map_err(|_| FailureAction11MaterialError::ChainAuthority)?
    {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    Ok(recovery)
}

fn ordered_action11_metas(
    accounts: &[&ObservedRpcAccount; FAILURE_ACTION11_ACCOUNT_COUNT_V1],
) -> Result<Vec<AccountMeta>> {
    const WRITABLE: [bool; FAILURE_ACTION11_ACCOUNT_COUNT_V1] = [
        false, false, false, true, true, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false, true, true, false,
    ];
    let keeper_refund_alias = accounts[40].address == accounts[41].address;
    Ok(accounts
        .iter()
        .enumerate()
        .map(|(index, account)| AccountMeta {
            pubkey: account.address,
            is_signer: false,
            is_writable: WRITABLE[index] || keeper_refund_alias && index == 41,
        })
        .collect())
}

fn snapshot_digest(accounts: &[&ObservedRpcAccount; FAILURE_ACTION11_ACCOUNT_COUNT_V1]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dragons-clutch/operator/failure-action11-finalized-snapshot/v1");
    hasher.update(accounts[0].provenance.slot.to_le_bytes());
    for account in accounts {
        hasher.update(account.address.to_bytes());
        hasher.update(account.owner.to_bytes());
        hasher.update(account.lamports.to_le_bytes());
        hasher.update([u8::from(account.executable)]);
        hasher.update((account.data.len() as u64).to_le_bytes());
        hasher.update(&account.data);
    }
    hasher.finalize().into()
}

fn require_pda(
    program_id: Address,
    observed: Address,
    bump: u8,
    seeds: &[&[u8]],
) -> Result<()> {
    let (expected, expected_bump) = Address::find_program_address(seeds, &program_id);
    if observed != expected || bump != expected_bump {
        return Err(FailureAction11MaterialError::ChainAuthority);
    }
    Ok(())
}

fn derive_recipe(program_id: Address, recipe: PdaRecipeV3) -> Result<RuntimeDerivedPdaV1> {
    recipe.validate().map_err(|_| FailureAction11MaterialError::ChainAuthority)?;
    let mut seeds = Vec::with_capacity(usize::from(recipe.seed_count()));
    let mut index = 0usize;
    while index < usize::from(recipe.seed_count()) {
        seeds.push(recipe.seed(index).map_err(|_| FailureAction11MaterialError::ChainAuthority)?);
        index += 1;
    }
    let (derived, bump) = Address::find_program_address(&seeds, &program_id);
    Ok(RuntimeDerivedPdaV1 {
        program_id: runtime_key(program_id),
        recipe_id: recipe.id().map_err(|_| FailureAction11MaterialError::ChainAuthority)?,
        address: runtime_key(derived),
        bump,
    })
}

fn account_view(account: &ObservedRpcAccount) -> RuntimeAccountViewV1<'_> {
    RuntimeAccountViewV1 {
        key: runtime_key(account.address),
        owner: runtime_key(account.owner),
        lamports: account.lamports,
        executable: account.executable,
        writable: false,
        signer: false,
        data: &account.data,
    }
}

fn runtime_key(address: Address) -> RuntimeKey {
    RuntimeKey::from_bytes(address.to_bytes())
}

fn map_construction(_error: ConstructionError) -> FailureAction11MaterialError {
    FailureAction11MaterialError::Construction
}
