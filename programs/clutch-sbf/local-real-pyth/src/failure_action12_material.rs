//! Finalized-chain unsigned material for current Failure action 12.
//!
//! Resolution carries an empty caller payload. The exact 64-role tuple,
//! sequence, completed consensus work, current Product/Source graph, inactive
//! ResolutionV5, replay funding authority, and v0 lookup table are recovered
//! from one finalized snapshot.

use crate::action_material::StructuredAddressLookupTableV1;
use crate::failure_action11_material::{
    authenticate_recovery, authenticate_registry_and_product, authenticate_source,
    FailureAction11ChainSnapshotV1,
};
use crate::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    RpcCommitment,
};
use crate::transaction_builder::{
    ConstructionError, ExactEquation, IntegerUnit, OwnedInstructionDraft,
    ProtocolTransactionBuilder, SemanticOwner, TransactionTransport,
    UnsignedProtocolTransaction,
};
use clutch_collateral_adapter_v2::{ResolutionStateV5, ResolutionV5};
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    FailureMarketIntervalCellPhaseV2, FailureMarketIntervalCellV2,
};
use clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryV2;
use clutch_failure_policy_runtime::market_policy_v1::FailureMarketAdmissionStateV1;
use clutch_failure_policy_runtime::market_quote_v1::FailureMarketRecoveryQuoteScheduleV1;
use clutch_failure_policy_runtime::market_replay_v2::{
    decode_and_reopen_failure_market_replay_from_chain_v2, FailureMarketReplayPhaseV2,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    FailureMarketRuntimePhaseV1, FailureMarketRuntimeV1,
};
use clutch_product_series::{
    ContentId, MarketLifecyclePhaseV3, SeriesFundingPhaseV5, SeriesMarketLinkPhaseV3,
};
use clutch_solana_layout::failure_market_interval_v2::{
    FailureMarketIntervalCellAccountV2, FailureMarketIntervalHistoryAccountV2,
};
use clutch_solana_layout::failure_market_replay_v2::FailureMarketReplayAccountV2;
use clutch_solana_layout::failure_recovery::{
    FailureMarketRootAccountV3, FailureMarketRuntimeRootAccountV1,
    FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3, FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesFundingAccountV5, SeriesMarketLinkAccountV3,
};
use clutch_solana_layout::registry::{self, ExtensionFamily, RecoveryAction};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;

pub const FAILURE_ACTION12_VALIDITY_SLOTS_V1: u64 = 16;
pub const FAILURE_ACTION12_ACCOUNT_COUNT_V1: usize = 64;
pub const FAILURE_ACTION12_LOCAL_ACTION_V1: u8 = 12;
pub const FAILURE_ACTION12_ROLE_LABELS_V1: [&str; FAILURE_ACTION12_ACCOUNT_COUNT_V1] = [
    "market-lifecycle-root", "series-market-link", "series-funding-v5",
    "failure-admission-root", "failure-runtime-root", "failure-interval-cell",
    "failure-interval-history", "failure-market-replay", "series-registry-v4",
    "registry-program", "registry-program-data", "registry-release-v2",
    "capability-profile-v4", "compiler-bundle-v7", "funding-quote-v6",
    "series-plan-v5", "series-funding-terms-v2", "product-template-v4",
    "native-claim-basis-v1", "recovery-policy-v1", "price-measure-policy-v1",
    "market-genesis-v2", "attachment-plan-v6", "market-instance-v2",
    "source-release-v2", "source-adapter-program", "source-adapter-program-data",
    "source-parser-program", "source-parser-program-data", "source-parser-config",
    "source-spec", "source-work-schedule", "source-receiver-program",
    "source-receiver-program-data", "source-receiver-config", "source-occurrence",
    "source-window", "source-statistic-key", "source-summary", "source-window-seal",
    "source-statistic-result", "source-result-lineage", "source-handoff-receipt",
    "source-work-receipt", "realm", "collateral-profile", "collateral-policy-release",
    "collateral-token-program", "general-market-binding-v5", "general-market-runtime-v3",
    "resolution-v5", "hoard-v2", "claim-ledger-v3", "source-terminal-policy",
    "source-terminal-receipt", "source-liveness-policy", "source-liveness-compartment",
    "source-funding-custody", "source-neutral-sink", "failure-liveness-policy",
    "failure-recovery-compartment", "recovery-refund-owner", "rent-sysvar",
    "system-program",
];
pub const FAILURE_ACTION12_ROLE_WRITABLE_V1: [bool; FAILURE_ACTION12_ACCOUNT_COUNT_V1] = [
    true, true, false, false, true, true, true, true, false, false, false, false,
    false, false, false, false, false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false, false, false, false, false,
    false, false, false, false, true, true, false, false, false, false, false, false,
    false, false, true, true, true, true, true, false, true, true, true, true, true, true,
    false, false,
];

const OWNER_PACKAGE: &str = "clutch-failure-policy-runtime+clutch-product-series+clutch-source-plane-v3-runtime+clutch-collateral-adapter-v2";
const OWNER_SCHEMA: &str = "dragons-clutch/operator/failure-action12-physical-resolution/v1";
const SEED_MARKET_ROOT: &[u8] = b"dc:market-lifecycle-root:v1";
const SEED_SERIES_LINK: &[u8] = b"dc:series-market-link:v1";
const SEED_SERIES_FUNDING: &[u8] = b"dc:series-funding:v1";
const SEED_FAILURE_ADMISSION: &[u8] = b"dc:failure-market-root:v2";
const SEED_FAILURE_RUNTIME: &[u8] = b"dc:failure-root:v2";
const SEED_FAILURE_CELL: &[u8] = b"dc:fail-int-cell:v2";
const SEED_FAILURE_HISTORY: &[u8] = b"dc:fail-int-history:v2";
const SEED_FAILURE_REPLAY: &[u8] = b"dc:failure-market-replay:v2";
const SEED_RESOLUTION: &[u8] = b"dc:resolution:v5";

pub type FailureAction12MaterialResult<T> =
    core::result::Result<T, FailureAction12MaterialError>;
type Result<T> = FailureAction12MaterialResult<T>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureAction12MaterialError {
    CheckedRelease,
    ChainSnapshot,
    ChainAuthority,
    ConsensusIncomplete,
    Arithmetic,
    Construction,
}

impl core::fmt::Display for FailureAction12MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit current Failure action 12",
            Self::ChainSnapshot => "Failure action-12 accounts are not one finalized snapshot",
            Self::ChainAuthority => "current Product/Failure/Source/collateral resolution authority refused",
            Self::ConsensusIncomplete => "Failure interval consensus is not complete",
            Self::Arithmetic => "Failure action-12 exact arithmetic overflowed",
            Self::Construction => "release-bound Failure action-12 construction refused",
        })
    }
}

impl std::error::Error for FailureAction12MaterialError {}

/// Every current action-12 role is named; callers cannot supply a numeric DTO.
#[derive(Clone, Copy, Debug)]
pub struct FailureAction12ChainSnapshotV1<'a> {
    pub market_lifecycle_root: &'a ObservedRpcAccount,
    pub series_market_link: &'a ObservedRpcAccount,
    pub series_funding: &'a ObservedRpcAccount,
    pub failure_admission_root: &'a ObservedRpcAccount,
    pub failure_runtime_root: &'a ObservedRpcAccount,
    pub failure_interval_cell: &'a ObservedRpcAccount,
    pub failure_interval_history: &'a ObservedRpcAccount,
    pub failure_market_replay: &'a ObservedRpcAccount,
    pub series_registry: &'a ObservedRpcAccount,
    pub registry_program: &'a ObservedRpcAccount,
    pub registry_program_data: &'a ObservedRpcAccount,
    pub registry_release_artifact: &'a ObservedRpcAccount,
    pub capability_profile_artifact: &'a ObservedRpcAccount,
    pub compiler_bundle_artifact: &'a ObservedRpcAccount,
    pub funding_quote_artifact: &'a ObservedRpcAccount,
    pub series_plan_artifact: &'a ObservedRpcAccount,
    pub series_funding_terms_artifact: &'a ObservedRpcAccount,
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
    pub source_receiver_program: &'a ObservedRpcAccount,
    pub source_receiver_program_data: &'a ObservedRpcAccount,
    pub source_receiver_config: &'a ObservedRpcAccount,
    pub source_occurrence: &'a ObservedRpcAccount,
    pub source_window: &'a ObservedRpcAccount,
    pub source_statistic_key: &'a ObservedRpcAccount,
    pub source_summary: &'a ObservedRpcAccount,
    pub source_window_seal: &'a ObservedRpcAccount,
    pub source_statistic_result: &'a ObservedRpcAccount,
    pub source_result_lineage: &'a ObservedRpcAccount,
    pub source_handoff_receipt: &'a ObservedRpcAccount,
    pub source_work_receipt: &'a ObservedRpcAccount,
    pub realm: &'a ObservedRpcAccount,
    pub collateral_profile: &'a ObservedRpcAccount,
    pub collateral_policy_release: &'a ObservedRpcAccount,
    pub collateral_token_program: &'a ObservedRpcAccount,
    pub general_market_binding: &'a ObservedRpcAccount,
    pub general_market_runtime: &'a ObservedRpcAccount,
    pub resolution: &'a ObservedRpcAccount,
    pub hoard: &'a ObservedRpcAccount,
    pub claim_ledger: &'a ObservedRpcAccount,
    pub source_terminal_policy: &'a ObservedRpcAccount,
    pub source_terminal_receipt: &'a ObservedRpcAccount,
    pub source_liveness_policy: &'a ObservedRpcAccount,
    pub source_liveness_compartment: &'a ObservedRpcAccount,
    pub source_funding_custody: &'a ObservedRpcAccount,
    pub source_neutral_sink: &'a ObservedRpcAccount,
    pub failure_liveness_policy: &'a ObservedRpcAccount,
    pub failure_recovery_compartment: &'a ObservedRpcAccount,
    pub recovery_refund_owner: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
    pub system_program: &'a ObservedRpcAccount,
    /// Finalized compression surface; never an instruction role.
    pub address_lookup_table: &'a ObservedRpcAccount,
}

impl<'a> FailureAction12ChainSnapshotV1<'a> {
    fn ordered(self) -> [&'a ObservedRpcAccount; FAILURE_ACTION12_ACCOUNT_COUNT_V1] {
        [
            self.market_lifecycle_root, self.series_market_link, self.series_funding,
            self.failure_admission_root, self.failure_runtime_root, self.failure_interval_cell,
            self.failure_interval_history, self.failure_market_replay, self.series_registry,
            self.registry_program, self.registry_program_data, self.registry_release_artifact,
            self.capability_profile_artifact, self.compiler_bundle_artifact,
            self.funding_quote_artifact, self.series_plan_artifact,
            self.series_funding_terms_artifact, self.product_template_artifact,
            self.native_claim_basis_artifact, self.recovery_policy_artifact,
            self.price_measure_policy_artifact, self.market_genesis_artifact,
            self.attachment_plan_artifact, self.market_instance_artifact, self.source_release,
            self.source_adapter_program, self.source_adapter_program_data,
            self.source_parser_program, self.source_parser_program_data,
            self.source_parser_config, self.source_spec, self.source_work_schedule,
            self.source_receiver_program, self.source_receiver_program_data,
            self.source_receiver_config, self.source_occurrence, self.source_window,
            self.source_statistic_key, self.source_summary, self.source_window_seal,
            self.source_statistic_result, self.source_result_lineage, self.source_handoff_receipt,
            self.source_work_receipt, self.realm, self.collateral_profile,
            self.collateral_policy_release, self.collateral_token_program,
            self.general_market_binding, self.general_market_runtime, self.resolution, self.hoard,
            self.claim_ledger, self.source_terminal_policy, self.source_terminal_receipt,
            self.source_liveness_policy, self.source_liveness_compartment,
            self.source_funding_custody, self.source_neutral_sink, self.failure_liveness_policy,
            self.failure_recovery_compartment, self.recovery_refund_owner, self.rent_sysvar,
            self.system_program,
        ]
    }

    fn action11_projection(self) -> FailureAction11ChainSnapshotV1<'a> {
        FailureAction11ChainSnapshotV1 {
            market_lifecycle_root: self.market_lifecycle_root,
            series_market_link: self.series_market_link,
            series_funding: self.series_funding,
            failure_admission_root: self.failure_admission_root,
            failure_runtime_root: self.failure_runtime_root,
            failure_interval_cell: self.failure_interval_cell,
            failure_interval_history: self.failure_interval_history,
            series_registry: self.series_registry,
            registry_program: self.registry_program,
            registry_program_data: self.registry_program_data,
            registry_release_artifact: self.registry_release_artifact,
            capability_profile_artifact: self.capability_profile_artifact,
            compiler_bundle_artifact: self.compiler_bundle_artifact,
            funding_quote_artifact: self.funding_quote_artifact,
            series_plan_artifact: self.series_plan_artifact,
            series_funding_terms_artifact: self.series_funding_terms_artifact,
            product_template_artifact: self.product_template_artifact,
            native_claim_basis_artifact: self.native_claim_basis_artifact,
            recovery_policy_artifact: self.recovery_policy_artifact,
            price_measure_policy_artifact: self.price_measure_policy_artifact,
            market_genesis_artifact: self.market_genesis_artifact,
            attachment_plan_artifact: self.attachment_plan_artifact,
            market_instance_artifact: self.market_instance_artifact,
            source_release: self.source_release,
            source_adapter_program: self.source_adapter_program,
            source_adapter_program_data: self.source_adapter_program_data,
            source_parser_program: self.source_parser_program,
            source_parser_program_data: self.source_parser_program_data,
            source_parser_config: self.source_parser_config,
            source_spec: self.source_spec,
            source_work_schedule: self.source_work_schedule,
            source_occurrence: self.source_occurrence,
            source_window: self.source_window,
            source_statistic_key: self.source_statistic_key,
            source_summary: self.source_summary,
            source_window_seal: self.source_window_seal,
            source_statistic_result: self.source_statistic_result,
            source_result_lineage: self.source_result_lineage,
            source_handoff_receipt: self.source_handoff_receipt,
            source_work_receipt: self.source_work_receipt,
            failure_liveness_policy: self.failure_liveness_policy,
            failure_recovery_compartment: self.failure_recovery_compartment,
            keeper: self.source_funding_custody,
            recovery_refund_owner: self.recovery_refund_owner,
            address_lookup_table: self.address_lookup_table,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChainDerivedFailureAction12MaterialV1 {
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
    resolved_coordinates: u64,
    state_sha256: [u8; 32],
    ordered_accounts: Vec<AccountMeta>,
    lookup_table: StructuredAddressLookupTableV1,
}

impl ChainDerivedFailureAction12MaterialV1 {
    pub const fn observed_slot(&self) -> u64 { self.observed_slot }
    pub const fn valid_before_slot(&self) -> u64 { self.valid_before_slot }
    pub const fn state_sha256(&self) -> [u8; 32] { self.state_sha256 }
    pub const fn sequence(&self) -> u64 { self.sequence }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn transition_nonce(&self) -> u64 { self.transition_nonce }
    pub(crate) fn driver_account(&self) -> Address { self.ordered_accounts[5].pubkey }
    pub(crate) fn account_metas(&self) -> &[AccountMeta] { &self.ordered_accounts }

    pub fn unsigned_instruction(&self, release: &IndexedProgramRelease) -> Result<OwnedInstructionDraft> {
        authenticate_material_release(self, release)?;
        OwnedInstructionDraft::checked_release_failure_action12_v1(
            release,
            SemanticOwner {
                package: OWNER_PACKAGE.into(),
                schema: OWNER_SCHEMA.into(),
                release_sha256: self.release_manifest_sha256,
            },
            self.ordered_accounts.clone(),
            vec![ExactEquation {
                name: "Completed consensus coordinates are finalized without caller funding".into(),
                unit: IntegerUnit::Count,
                left: u128::from(self.resolved_coordinates),
                right: u128::from(self.resolved_coordinates),
            }],
            self.sequence,
        ).map_err(map_construction)
    }

    pub fn unsigned_transaction(
        &self,
        release: &IndexedProgramRelease,
        payer: Address,
        transport: TransactionTransport,
    ) -> Result<UnsignedProtocolTransaction> {
        let draft = self.unsigned_instruction(release)?;
        ProtocolTransactionBuilder::new(payer, release.program_id, release.release_manifest_sha256, transport)
            .and_then(|builder| builder.build_exact_v0(
                draft, self.lookup_table.table(), self.lookup_table.observed_slot(),
                self.lookup_table.state_sha256(),
            )).map_err(map_construction)
    }

    pub(crate) fn build_unsigned_transaction(
        &self,
        release: &IndexedProgramRelease,
        builder: &ProtocolTransactionBuilder,
    ) -> Result<UnsignedProtocolTransaction> {
        let draft = self.unsigned_instruction(release)?;
        builder.build_exact_v0(
            draft, self.lookup_table.table(), self.lookup_table.observed_slot(),
            self.lookup_table.state_sha256(),
        ).map_err(map_construction)
    }
}

pub fn derive_failure_action12_material_v1(
    release: &IndexedProgramRelease,
    snapshot: FailureAction12ChainSnapshotV1<'_>,
) -> Result<ChainDerivedFailureAction12MaterialV1> {
    authenticate_release(release)?;
    let ordered = snapshot.ordered();
    authenticate_provenance(release, &ordered)?;
    authenticate_lookup_provenance(snapshot.address_lookup_table, ordered[0])?;
    authenticate_role_shapes(release, &ordered)?;
    let lookup_table = StructuredAddressLookupTableV1::authenticate(snapshot.address_lookup_table)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;

    let root_frame = MarketLifecycleRootAccountV3::decode(&snapshot.market_lifecycle_root.data)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let root = &root_frame.state;
    let root_binding = root.binding();
    if root.phase() != MarketLifecyclePhaseV3::Active
        || root.resolution_semantic_id() != ContentId::ZERO
        || root.resolution_data_id() != ContentId::ZERO
        || root.resolution_activation_receipt_id() != ContentId::ZERO
        || snapshot.market_lifecycle_root.lamports < root_frame.rent_principal_lamports
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.market_lifecycle_root.address, root_frame.stored_bump,
        &[SEED_MARKET_ROOT, &root_binding.market_instance_id.bytes(), &root_binding.generation.to_le_bytes()])?;

    let link_frame = SeriesMarketLinkAccountV3::decode(&snapshot.series_market_link.data)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let link = link_frame.state;
    let link_binding = link.binding();
    if link.phase() != SeriesMarketLinkPhaseV3::Active
        || link.active_failure_sessions() != 1
        || link_binding.market_instance_id != root_binding.market_instance_id
        || link_binding.generation != root_binding.generation
        || link_binding.market_root_account_id.bytes() != snapshot.market_lifecycle_root.address.to_bytes()
        || link_binding.market_binding_id != root_binding.id().map_err(|_| FailureAction12MaterialError::ChainAuthority)?
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.series_market_link.address, link_frame.stored_bump,
        &[SEED_SERIES_LINK, &link_binding.series_plan_id.bytes(), &link_binding.ordinal.to_le_bytes()])?;

    let funding_frame = SeriesFundingAccountV5::decode(&snapshot.series_funding.data)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let funding = funding_frame.state;
    if snapshot.series_funding.address.to_bytes() != link_binding.funding_state_account_id.bytes()
        || snapshot.series_funding.lamports < funding_frame.rent_principal_lamports
        || funding.series_plan_id != link_binding.series_plan_id
        || funding.funding_terms_id != link_binding.funding_terms_id
        || funding.funding_quote_id != link_binding.funding_quote_id
        || funding.attachment_plan_id != link_binding.attachment_plan_id
        || funding.compiler_bundle_id != link_binding.compiler_bundle_id
        || funding.phase == SeriesFundingPhaseV5::Pending
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.series_funding.address, funding_frame.stored_bump,
        &[SEED_SERIES_FUNDING, &link_binding.series_plan_id.bytes()])?;

    let admission_bytes: &[u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3] = snapshot.failure_admission_root.data.as_slice()
        .try_into().map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let admission_frame = FailureMarketRootAccountV3::decode(admission_bytes)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let admission = FailureMarketAdmissionStateV1::decode(&admission_frame.admission_body)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let policy = admission.binding().facts();
    let quote = FailureMarketRecoveryQuoteScheduleV1::decode(&admission_frame.recovery_quote_body)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    if policy.market_instance_id != root_binding.market_instance_id
        || policy.generation != root_binding.generation
        || root_binding.market_failure_policy_binding_id.bytes() != admission.binding().id().bytes()
        || quote.id().map_err(|_| FailureAction12MaterialError::ChainAuthority)?.bytes()
            != policy.recovery_quote_schedule_id.bytes()
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.failure_admission_root.address, admission_frame.bump,
        &[SEED_FAILURE_ADMISSION, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()])?;

    let runtime_bytes: &[u8; FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1] = snapshot.failure_runtime_root.data.as_slice()
        .try_into().map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let runtime_frame = FailureMarketRuntimeRootAccountV1::decode(runtime_bytes)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let runtime = FailureMarketRuntimeV1::decode_for_admission(&runtime_frame.runtime_body, admission)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    if runtime.phase() != FailureMarketRuntimePhaseV1::IntervalActive
        || runtime.runtime_account_id().bytes() != snapshot.failure_runtime_root.address.to_bytes()
        || snapshot.failure_runtime_root.lamports < runtime.root_funding().observed_balance_lamports
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.failure_runtime_root.address, runtime_frame.bump,
        &[SEED_FAILURE_RUNTIME, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()])?;

    let cell_bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = snapshot.failure_interval_cell.data.as_slice()
        .try_into().map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let cell_frame = FailureMarketIntervalCellAccountV2::decode(cell_bytes)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let cell = FailureMarketIntervalCellV2::decode_canonical(cell_frame.semantic_body())
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let history_bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] = snapshot.failure_interval_history.data.as_slice()
        .try_into().map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let history_frame = FailureMarketIntervalHistoryAccountV2::decode(history_bytes)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let history = FailureMarketIntervalHistoryV2::decode_canonical(history_frame.semantic_body())
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    if cell.phase() != FailureMarketIntervalCellPhaseV2::Active
        || cell.failure_policy_binding_id() != admission.binding().id()
        || cell.market_instance_id() != policy.market_instance_id
        || cell.generation() != policy.generation
        || cell.history_account().bytes() != snapshot.failure_interval_history.address.to_bytes()
        || history.failure_policy_binding_id() != cell.failure_policy_binding_id()
        || history.market_instance_id() != cell.market_instance_id()
        || history.generation() != cell.generation()
        || history.funding_receipt_id() != cell.funding_receipt_id()
        || history.completed_session_count() != cell.completed_session_count()
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.failure_interval_cell.address, cell_frame.bump(),
        &[SEED_FAILURE_CELL, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()])?;
    require_pda(release.program_id, snapshot.failure_interval_history.address, history_frame.bump(),
        &[SEED_FAILURE_HISTORY, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()])?;
    let work = cell.product_work().map_err(|_| FailureAction12MaterialError::ChainAuthority)?
        .ok_or(FailureAction12MaterialError::ConsensusIncomplete)?;
    if !work.is_complete() || work.market_instance_id() != policy.market_instance_id {
        return Err(FailureAction12MaterialError::ConsensusIncomplete);
    }

    let replay_bytes: &[u8; registry::FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2] = snapshot.failure_market_replay.data.as_slice()
        .try_into().map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let replay_frame = FailureMarketReplayAccountV2::decode(replay_bytes)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    let (replay, replay_funding) = decode_and_reopen_failure_market_replay_from_chain_v2(
        replay_frame.semantic_body(), admission,
        clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1::from_bytes(
            snapshot.failure_market_replay.address.to_bytes(),
        ),
    ).map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    if replay.phase() != FailureMarketReplayPhaseV2::Pending
        || snapshot.failure_market_replay.lamports < replay_funding.facts().observed_balance_lamports
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.failure_market_replay.address, replay_frame.bump(),
        &[SEED_FAILURE_REPLAY, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()])?;

    let resolution = ResolutionV5::decode(&snapshot.resolution.data)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    if resolution.state != ResolutionStateV5::Inactive
        || resolution.facts.market_instance_id.bytes() != policy.market_instance_id.bytes()
        || resolution.facts.native_claim_basis_id.bytes() != root_binding.native_claim_basis_id.bytes()
        || resolution.facts.outcome_count != root_binding.outcome_count
        || resolution.facts.generation != policy.generation
        || resolution.facts.finalization_evidence_id.bytes() != [0; 32]
        || resolution.facts.payout_denominator != 0
        || resolution.facts.payout_weights.iter().any(|weight| *weight != 0)
        || snapshot.resolution.address.to_bytes() != root_binding.resolution_account_id.bytes()
        || snapshot.resolution.lamports < resolution.rent.refundable_principal
            .checked_add(resolution.rent.donation_floor).ok_or(FailureAction12MaterialError::Arithmetic)?
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.resolution.address, resolution.stored_bump,
        &[SEED_RESOLUTION, &policy.market_instance_id.bytes()])?;

    let projection = snapshot.action11_projection();
    authenticate_registry_and_product(release, projection, root_binding, link_binding)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    authenticate_source(release, projection, policy, root_binding, link_binding, cell)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    authenticate_recovery(release, projection, policy, quote)
        .map_err(|_| FailureAction12MaterialError::ChainAuthority)?;
    authenticate_receiver(snapshot)?;

    let sequence = runtime.transition_sequence().checked_add(1)
        .ok_or(FailureAction12MaterialError::Arithmetic)?;
    let observed_slot = snapshot.failure_admission_root.provenance.slot;
    let valid_before_slot = observed_slot.checked_add(FAILURE_ACTION12_VALIDITY_SLOTS_V1)
        .ok_or(FailureAction12MaterialError::Arithmetic)?;
    Ok(ChainDerivedFailureAction12MaterialV1 {
        checked_release_key: release.key(),
        program_id: release.program_id,
        program_data: release.program_data,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        observed_slot,
        valid_before_slot,
        generation: policy.generation,
        transition_nonce: cell.transition_nonce(),
        sequence,
        resolved_coordinates: work.checked_coordinates(),
        state_sha256: snapshot_digest(&ordered),
        ordered_accounts: ordered.iter().enumerate().map(|(index, account)| AccountMeta {
            pubkey: account.address,
            is_signer: false,
            is_writable: FAILURE_ACTION12_ROLE_WRITABLE_V1[index],
        }).collect(),
        lookup_table,
    })
}

fn authenticate_release(release: &IndexedProgramRelease) -> Result<()> {
    release.validate().map_err(|_| FailureAction12MaterialError::CheckedRelease)?;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: ExtensionFamily::Recovery.tag(),
        family_version: ExtensionFamily::Recovery.version(),
        local_action: RecoveryAction::ResolveIntervalConsensus.tag(),
    };
    if !release.families.contains(&CanonicalFamily::Failure)
        || release.enabled_intents.binary_search(&coordinate).is_err()
    {
        return Err(FailureAction12MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_material_release(material: &ChainDerivedFailureAction12MaterialV1, release: &IndexedProgramRelease) -> Result<()> {
    authenticate_release(release)?;
    if release.key() != material.checked_release_key
        || release.program_id != material.program_id
        || release.program_data != material.program_data
        || release.release_manifest_sha256 != material.release_manifest_sha256
        || release.capability_profile_id != material.capability_profile_id
    {
        return Err(FailureAction12MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_provenance(release: &IndexedProgramRelease, accounts: &[&ObservedRpcAccount; FAILURE_ACTION12_ACCOUNT_COUNT_V1]) -> Result<()> {
    let first = &accounts[0].provenance;
    let release_key = release.key();
    if first.commitment != RpcCommitment::Finalized || first.slot == 0 || first.release_key != release_key
        || accounts.iter().any(|account| account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot != first.slot || account.provenance.cluster_key != first.cluster_key
            || account.provenance.release_key != release_key)
    {
        return Err(FailureAction12MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn authenticate_lookup_provenance(lookup: &ObservedRpcAccount, first: &ObservedRpcAccount) -> Result<()> {
    if lookup.provenance.commitment != RpcCommitment::Finalized
        || lookup.provenance.slot != first.provenance.slot
        || lookup.provenance.cluster_key != first.provenance.cluster_key
        || lookup.provenance.release_key != first.provenance.release_key
    {
        return Err(FailureAction12MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn authenticate_role_shapes(release: &IndexedProgramRelease, accounts: &[&ObservedRpcAccount; FAILURE_ACTION12_ACCOUNT_COUNT_V1]) -> Result<()> {
    const EXECUTABLE: [usize; 6] = [9, 25, 27, 32, 47, 63];
    for (index, account) in accounts.iter().enumerate() {
        if account.address == Address::default() || account.executable != EXECUTABLE.contains(&index) {
            return Err(FailureAction12MaterialError::ChainAuthority);
        }
        for (other_index, other) in accounts[index + 1..].iter().enumerate() {
            let absolute = index + 1 + other_index;
            let allowed = (index == 57 && absolute == 61) || (index == 61 && absolute == 57);
            if account.address == other.address && !allowed {
                return Err(FailureAction12MaterialError::ChainAuthority);
            }
        }
    }
    for index in [0usize, 1, 2, 3, 4, 5, 6, 7, 8, 50, 51, 52] {
        if accounts[index].owner != release.program_id {
            return Err(FailureAction12MaterialError::ChainAuthority);
        }
    }
    Ok(())
}

fn authenticate_receiver(snapshot: FailureAction12ChainSnapshotV1<'_>) -> Result<()> {
    const PROGRAM_METADATA_BYTES: usize = 36;
    const PROGRAMDATA_METADATA_BYTES: usize = 45;
    if snapshot.source_receiver_program.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || snapshot.source_receiver_program_data.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || !snapshot.source_receiver_program.executable || snapshot.source_receiver_program_data.executable
        || snapshot.source_receiver_program.data.len() < PROGRAM_METADATA_BYTES
        || snapshot.source_receiver_program_data.data.len() < PROGRAMDATA_METADATA_BYTES
        || snapshot.source_receiver_program.data.get(..4) != Some(2_u32.to_le_bytes().as_slice())
        || snapshot.source_receiver_program_data.data.get(..4) != Some(3_u32.to_le_bytes().as_slice())
        || snapshot.source_receiver_program.data.get(4..36)
            != Some(snapshot.source_receiver_program_data.address.to_bytes().as_slice())
        || snapshot.source_receiver_config.owner != snapshot.source_receiver_program.address
        || snapshot.source_receiver_config.executable
        || snapshot.rent_sysvar.address != solana_sdk_ids::sysvar::rent::ID
        || snapshot.rent_sysvar.executable
        || snapshot.system_program.address != solana_sdk_ids::system_program::ID
        || !snapshot.system_program.executable
    {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    Ok(())
}

fn require_pda(program_id: Address, observed: Address, bump: u8, seeds: &[&[u8]]) -> Result<()> {
    let (expected, expected_bump) = Address::find_program_address(seeds, &program_id);
    if observed != expected || bump != expected_bump {
        return Err(FailureAction12MaterialError::ChainAuthority);
    }
    Ok(())
}

fn snapshot_digest(accounts: &[&ObservedRpcAccount; FAILURE_ACTION12_ACCOUNT_COUNT_V1]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dragons-clutch/operator/failure-action12-finalized-snapshot/v1");
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

fn map_construction(_: ConstructionError) -> FailureAction12MaterialError {
    FailureAction12MaterialError::Construction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_contract_matches_current_dispatch_width() {
        assert_eq!(FAILURE_ACTION12_ROLE_LABELS_V1.len(), 64);
        assert_eq!(FAILURE_ACTION12_ROLE_WRITABLE_V1.len(), 64);
        assert_eq!(FAILURE_ACTION12_ROLE_LABELS_V1[50], "resolution-v5");
        assert!(FAILURE_ACTION12_ROLE_WRITABLE_V1[50]);
    }
}
