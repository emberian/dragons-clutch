//! Finalized-chain unsigned material for current Failure action 13.
//!
//! The finite exhausted-session archive has an empty caller payload. Its
//! sequence, exhaustion boundary, exact 16-role tuple, privileges, release,
//! and lookup table are recovered from current RootV3/LinkV3/FundingV5,
//! Failure, Product registry, and liveness owners.

use crate::action_material::StructuredAddressLookupTableV1;
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
use clutch_failure_policy_runtime::market_quote_v1::FailureMarketRecoveryQuoteScheduleV1;
use clutch_failure_policy_runtime::market_runtime_v1::{
    FailureMarketRuntimePhaseV1, FailureMarketRuntimeV1,
};
use clutch_liveness::{RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1, RuntimeCompartmentV1, RuntimeLivenessPolicyV1};
use clutch_product_series::{
    CompiledProductSeriesBundleV7, FixedCodec, MarketLifecyclePhaseV3,
    RegistryCapabilityProfileV4, RegistryProgramReleaseV2, RegistryReleaseLocusV2,
    SeriesFundingPhaseV5, SeriesFundingQuoteV6, SeriesMarketLinkPhaseV3,
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
    MarketLifecycleRootAccountV3, SeriesFundingAccountV5, SeriesMarketLinkAccountV3,
    SeriesRegistryAccountV4,
};
use clutch_solana_layout::registry::{self, RecoveryAction};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;

pub const FAILURE_ACTION13_VALIDITY_SLOTS_V1: u64 = 32;
pub const FAILURE_ACTION13_ACCOUNT_COUNT_V1: usize = 16;
pub const FAILURE_ACTION13_ROLE_LABELS_V1: [&str; FAILURE_ACTION13_ACCOUNT_COUNT_V1] = [
    "market-lifecycle-root",
    "series-market-link",
    "series-funding-v5",
    "failure-admission-root",
    "failure-runtime-root",
    "failure-interval-cell",
    "failure-interval-history",
    "series-registry-v4",
    "registry-program",
    "registry-program-data",
    "registry-release-v2",
    "capability-profile-v4",
    "compiler-bundle-v7",
    "funding-quote-v6",
    "failure-liveness-policy",
    "failure-recovery-compartment",
];
pub const FAILURE_ACTION13_ROLE_WRITABLE_V1: [bool; FAILURE_ACTION13_ACCOUNT_COUNT_V1] = [
    false, true, false, false, true, true, true, false,
    false, false, false, false, false, false, false, false,
];

const OWNER_PACKAGE: &str = "clutch-failure-policy-runtime+clutch-product-series";
const OWNER_SCHEMA: &str = "dragons-clutch/operator/failure-action13-finite-session-archive/v1";
const SEED_PRODUCT_ARTIFACT: &[u8] = b"dc:product-artifact:v1";
const SEED_MARKET_ROOT: &[u8] = b"dc:market-lifecycle-root:v1";
const SEED_SERIES_LINK: &[u8] = b"dc:series-market-link:v1";
const SEED_SERIES_REGISTRY: &[u8] = b"dc:series-registry:v1";
const SEED_SERIES_FUNDING: &[u8] = b"dc:series-funding:v1";
const SEED_FAILURE_ADMISSION: &[u8] = b"dc:failure-market-root:v2";
const SEED_FAILURE_RUNTIME: &[u8] = b"dc:failure-root:v2";
const SEED_FAILURE_CELL: &[u8] = b"dc:fail-int-cell:v2";
const SEED_FAILURE_HISTORY: &[u8] = b"dc:fail-int-history:v2";
const SEED_FAILURE_POLICY: &[u8] = b"dc:failure-live-policy:v1";
const SEED_FAILURE_RECOVERY: &[u8] = b"dc:failure-recovery:v1";

pub type FailureAction13MaterialResult<T> =
    core::result::Result<T, FailureAction13MaterialError>;
type Result<T> = FailureAction13MaterialResult<T>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureAction13MaterialError {
    CheckedRelease,
    ChainSnapshot,
    ChainAuthority,
    NotExhausted,
    Arithmetic,
    Construction,
}

impl core::fmt::Display for FailureAction13MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit current Failure action 13",
            Self::ChainSnapshot => "Failure action-13 accounts are not one finalized snapshot",
            Self::ChainAuthority => "current Failure/Product/liveness archive authority refused",
            Self::NotExhausted => "active Failure session has not reached its canonical finite exhaustion boundary",
            Self::Arithmetic => "Failure action-13 exact arithmetic overflowed",
            Self::Construction => "release-bound Failure action-13 construction refused",
        })
    }
}

impl std::error::Error for FailureAction13MaterialError {}

#[derive(Clone, Copy, Debug)]
pub struct FailureAction13ChainSnapshotV1<'a> {
    pub market_lifecycle_root: &'a ObservedRpcAccount,
    pub series_market_link: &'a ObservedRpcAccount,
    pub series_funding: &'a ObservedRpcAccount,
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
    pub failure_liveness_policy: &'a ObservedRpcAccount,
    pub failure_recovery_compartment: &'a ObservedRpcAccount,
    /// Finalized compression surface; never an instruction role or authority.
    pub address_lookup_table: &'a ObservedRpcAccount,
}

impl<'a> FailureAction13ChainSnapshotV1<'a> {
    fn ordered(self) -> [&'a ObservedRpcAccount; FAILURE_ACTION13_ACCOUNT_COUNT_V1] {
        [
            self.market_lifecycle_root,
            self.series_market_link,
            self.series_funding,
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
            self.failure_liveness_policy,
            self.failure_recovery_compartment,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct ChainDerivedFailureAction13MaterialV1 {
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
    recovery_principal_lamports: u64,
    state_sha256: [u8; 32],
    ordered_accounts: Vec<AccountMeta>,
    lookup_table: StructuredAddressLookupTableV1,
}

impl ChainDerivedFailureAction13MaterialV1 {
    pub const fn observed_slot(&self) -> u64 { self.observed_slot }
    pub const fn valid_before_slot(&self) -> u64 { self.valid_before_slot }
    pub const fn state_sha256(&self) -> [u8; 32] { self.state_sha256 }
    pub const fn sequence(&self) -> u64 { self.sequence }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn transition_nonce(&self) -> u64 { self.transition_nonce }
    pub(crate) fn driver_account(&self) -> Address { self.ordered_accounts[5].pubkey }
    pub(crate) fn account_metas(&self) -> &[AccountMeta] { &self.ordered_accounts }

    pub fn unsigned_instruction(
        &self,
        release: &IndexedProgramRelease,
    ) -> Result<OwnedInstructionDraft> {
        authenticate_material_release(self, release)?;
        OwnedInstructionDraft::checked_release_failure_action13_v1(
            release,
            SemanticOwner {
                package: OWNER_PACKAGE.into(),
                schema: OWNER_SCHEMA.into(),
                release_sha256: self.release_manifest_sha256,
            },
            self.ordered_accounts.clone(),
            vec![ExactEquation {
                name: "Finite-session archive preserves Recovery custody work principal".into(),
                unit: IntegerUnit::Lamports,
                left: u128::from(self.recovery_principal_lamports),
                right: u128::from(self.recovery_principal_lamports),
            }],
            self.sequence,
        )
        .map_err(map_construction)
    }

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
        .and_then(|builder| {
            builder.build_exact_v0(
                draft,
                self.lookup_table.table(),
                self.lookup_table.observed_slot(),
                self.lookup_table.state_sha256(),
            )
        })
        .map_err(map_construction)
    }

    pub(crate) fn build_unsigned_transaction(
        &self,
        release: &IndexedProgramRelease,
        builder: &ProtocolTransactionBuilder,
    ) -> Result<UnsignedProtocolTransaction> {
        let draft = self.unsigned_instruction(release)?;
        builder
            .build_exact_v0(
                draft,
                self.lookup_table.table(),
                self.lookup_table.observed_slot(),
                self.lookup_table.state_sha256(),
            )
            .map_err(map_construction)
    }
}

pub fn derive_failure_action13_material_v1(
    release: &IndexedProgramRelease,
    snapshot: FailureAction13ChainSnapshotV1<'_>,
) -> Result<ChainDerivedFailureAction13MaterialV1> {
    authenticate_release(release)?;
    let ordered = snapshot.ordered();
    authenticate_provenance(release, &ordered)?;
    authenticate_lookup_provenance(snapshot.address_lookup_table, ordered[0])?;
    authenticate_role_shapes(release, &ordered)?;
    let lookup_table = StructuredAddressLookupTableV1::authenticate(snapshot.address_lookup_table)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;

    let root_frame = MarketLifecycleRootAccountV3::decode(&snapshot.market_lifecycle_root.data)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let root = &root_frame.state;
    let root_binding = root.binding();
    if root.phase() != MarketLifecyclePhaseV3::Active
        || root.resolution_semantic_id() != clutch_product_series::ContentId::ZERO
        || root.resolution_data_id() != clutch_product_series::ContentId::ZERO
        || root.resolution_activation_receipt_id() != clutch_product_series::ContentId::ZERO
        || snapshot.market_lifecycle_root.lamports < root_frame.rent_principal_lamports
    {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.market_lifecycle_root.address,
        root_frame.stored_bump,
        &[SEED_MARKET_ROOT, &root_binding.market_instance_id.bytes(), &root_binding.generation.to_le_bytes()],
    )?;

    let link_frame = SeriesMarketLinkAccountV3::decode(&snapshot.series_market_link.data)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let link = link_frame.state;
    let link_binding = link.binding();
    if link.phase() != SeriesMarketLinkPhaseV3::Active
        || link.active_failure_sessions() != 1
        || link.failure_sessions_started() == 0
        || link_binding.market_instance_id != root_binding.market_instance_id
        || link_binding.generation != root_binding.generation
        || link_binding.market_root_account_id.bytes() != snapshot.market_lifecycle_root.address.to_bytes()
        || link_binding.market_binding_id != root_binding.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?
    {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.series_market_link.address,
        link_frame.stored_bump,
        &[SEED_SERIES_LINK, &link_binding.series_plan_id.bytes(), &link_binding.ordinal.to_le_bytes()],
    )?;

    let funding_frame = SeriesFundingAccountV5::decode(&snapshot.series_funding.data)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
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
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.series_funding.address,
        funding_frame.stored_bump,
        &[SEED_SERIES_FUNDING, &link_binding.series_plan_id.bytes()],
    )?;

    let admission_bytes: &[u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3] = snapshot
        .failure_admission_root.data.as_slice().try_into()
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let admission_frame = FailureMarketRootAccountV3::decode(admission_bytes)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let admission = FailureMarketAdmissionStateV1::decode(&admission_frame.admission_body)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let policy = admission.binding().facts();
    let recovery_quote = FailureMarketRecoveryQuoteScheduleV1::decode(&admission_frame.recovery_quote_body)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    if root_binding.market_failure_policy_binding_id.bytes() != admission.binding().id().bytes()
        || policy.market_instance_id != root_binding.market_instance_id
        || policy.generation != root_binding.generation
        || policy.capability_profile_id.bytes() != root_binding.capability_profile_id.bytes()
        || recovery_quote.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.bytes()
            != policy.recovery_quote_schedule_id.bytes()
    {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.failure_admission_root.address,
        admission_frame.bump,
        &[SEED_FAILURE_ADMISSION, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()],
    )?;

    let runtime_bytes: &[u8; FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1] = snapshot
        .failure_runtime_root.data.as_slice().try_into()
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let runtime_frame = FailureMarketRuntimeRootAccountV1::decode(runtime_bytes)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let runtime = FailureMarketRuntimeV1::decode_for_admission(&runtime_frame.runtime_body, admission)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    if runtime.phase() != FailureMarketRuntimePhaseV1::IntervalActive
        || runtime.runtime_account_id().bytes() != snapshot.failure_runtime_root.address.to_bytes()
        || snapshot.failure_runtime_root.lamports < runtime.root_funding().observed_balance_lamports
    {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    require_pda(
        release.program_id,
        snapshot.failure_runtime_root.address,
        runtime_frame.bump,
        &[SEED_FAILURE_RUNTIME, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()],
    )?;

    let cell_bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = snapshot
        .failure_interval_cell.data.as_slice().try_into()
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let cell_frame = FailureMarketIntervalCellAccountV2::decode(cell_bytes)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let cell = FailureMarketIntervalCellV2::decode_canonical(cell_frame.semantic_body())
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let history_bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] = snapshot
        .failure_interval_history.data.as_slice().try_into()
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let history_frame = FailureMarketIntervalHistoryAccountV2::decode(history_bytes)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let history = FailureMarketIntervalHistoryV2::decode_canonical(history_frame.semantic_body())
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
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
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.failure_interval_cell.address, cell_frame.bump(), &[SEED_FAILURE_CELL, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()])?;
    require_pda(release.program_id, snapshot.failure_interval_history.address, history_frame.bump(), &[SEED_FAILURE_HISTORY, &policy.market_instance_id.bytes(), &policy.generation.to_le_bytes()])?;

    authenticate_product_subset(release, snapshot, root_binding, link_binding, policy)?;
    let recovery = authenticate_recovery(release, snapshot, policy, recovery_quote)?;
    let work = cell.product_work().map_err(|_| FailureAction13MaterialError::ChainAuthority)?
        .ok_or(FailureAction13MaterialError::NotExhausted)?;
    if work.is_complete() {
        return Err(FailureAction13MaterialError::NotExhausted);
    }
    let attempt = recovery_quote.attempts.get(usize::from(cell.attempt_index()))
        .ok_or(FailureAction13MaterialError::ChainAuthority)?;
    let aggregate_calls = history.completed_work_calls().checked_add(cell.completed_work_calls())
        .ok_or(FailureAction13MaterialError::Arithmetic)?;
    let aggregate_rewards = history.exact_reward_lamports().checked_add(cell.exact_reward_lamports())
        .ok_or(FailureAction13MaterialError::Arithmetic)?;
    let principal = recovery_quote.work_principal_lamports()
        .map_err(|_| FailureAction13MaterialError::Arithmetic)?;
    if aggregate_calls != u64::from(recovery.completed_calls)
        || aggregate_rewards != recovery.keeper_paid_lamports
        || recovery.completed_work_ceiling_lamports != recovery.keeper_paid_lamports
        || !(
            cell.accepted_progress_units() == attempt.max_progress_units
                || aggregate_calls == u64::from(recovery_quote.maximum_calls)
                || aggregate_rewards == principal
        )
    {
        return Err(FailureAction13MaterialError::NotExhausted);
    }
    let sequence = runtime.transition_sequence().checked_add(1)
        .ok_or(FailureAction13MaterialError::Arithmetic)?;
    let observed_slot = snapshot.failure_admission_root.provenance.slot;
    let valid_before_slot = observed_slot.checked_add(FAILURE_ACTION13_VALIDITY_SLOTS_V1)
        .ok_or(FailureAction13MaterialError::Arithmetic)?;
    Ok(ChainDerivedFailureAction13MaterialV1 {
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
        recovery_principal_lamports: principal,
        state_sha256: snapshot_digest(&ordered),
        ordered_accounts: ordered.iter().enumerate().map(|(index, account)| AccountMeta {
            pubkey: account.address,
            is_signer: false,
            is_writable: FAILURE_ACTION13_ROLE_WRITABLE_V1[index],
        }).collect(),
        lookup_table,
    })
}

fn authenticate_product_subset(
    release: &IndexedProgramRelease,
    snapshot: FailureAction13ChainSnapshotV1<'_>,
    root: clutch_product_series::MarketLifecycleBindingV3,
    link: clutch_product_series::SeriesMarketLinkBindingV3,
    policy: clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
) -> Result<()> {
    let registry = SeriesRegistryAccountV4::decode(&snapshot.series_registry.data)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    if registry.series_plan_id != link.series_plan_id
        || registry.funding_terms_id != link.funding_terms_id
        || registry.registry_release_id != root.registry_release_id
        || registry.capability_profile_id != root.capability_profile_id
        || registry.compiler_bundle_id != link.compiler_bundle_id
        || snapshot.series_registry.lamports < registry.rent_principal_lamports
        || !registry.activation_consumed
    {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    require_pda(release.program_id, snapshot.series_registry.address, registry.stored_bump, &[SEED_SERIES_REGISTRY, &link.series_plan_id.bytes()])?;

    let registry_release = RegistryProgramReleaseV2::decode(&snapshot.registry_release_artifact.data)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let profile = RegistryCapabilityProfileV4::decode(&snapshot.capability_profile_artifact.data)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let bundle = CompiledProductSeriesBundleV7::decode(&snapshot.compiler_bundle_artifact.data)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let quote = SeriesFundingQuoteV6::decode(&snapshot.funding_quote_artifact.data)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    require_artifact(release, snapshot.registry_release_artifact, ArtifactKind::RegistryProgramReleaseV2, registry_release.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.content_id().bytes())?;
    require_artifact(release, snapshot.capability_profile_artifact, ArtifactKind::RegistryCapabilityProfileV4, profile.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.content_id().bytes())?;
    require_artifact(release, snapshot.compiler_bundle_artifact, ArtifactKind::CompiledProductSeriesBundleV7, bundle.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.content_id().bytes())?;
    require_artifact(release, snapshot.funding_quote_artifact, ArtifactKind::SeriesFundingQuoteV6, quote.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.content_id().bytes())?;
    if registry_release.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.content_id().bytes() != root.registry_release_id.bytes()
        || profile.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.content_id().bytes() != root.capability_profile_id.bytes()
        || bundle.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.content_id().bytes() != link.compiler_bundle_id.bytes()
        || quote.id().map_err(|_| FailureAction13MaterialError::ChainAuthority)?.content_id().bytes() != link.funding_quote_id.bytes()
        || registry_release.program.bytes() != snapshot.registry_program.address.to_bytes()
        || registry_release.programdata.bytes() != snapshot.registry_program_data.address.to_bytes()
        || profile.rules.registry_release_id != root.registry_release_id
        || bundle.series_plan_id != link.series_plan_id
        || bundle.funding_terms_id != link.funding_terms_id
        || bundle.funding_quote_id != link.funding_quote_id
        || bundle.attachment_plan_id != link.attachment_plan_id
        || quote.failure_liveness_policy_id.bytes() != policy.liveness_policy_id.bytes()
        || quote.failure_recovery_quote_schedule_id.bytes() != policy.recovery_quote_schedule_id.bytes()
    {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    authenticate_loader_pair(snapshot.registry_program, snapshot.registry_program_data, registry_release)
}

fn authenticate_recovery(
    release: &IndexedProgramRelease,
    snapshot: FailureAction13ChainSnapshotV1<'_>,
    policy: clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
    quote: FailureMarketRecoveryQuoteScheduleV1,
) -> Result<RuntimeCompartmentV1> {
    let policy_frame = decode_failure_account_body_v1(&snapshot.failure_liveness_policy.data, registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG, registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION, FAILURE_LIVENESS_POLICY_BODY_BYTES_V1)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let liveness_policy = RuntimeLivenessPolicyV1::decode(policy_frame.body)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let recovery_frame = decode_failure_account_body_v1(&snapshot.failure_recovery_compartment.data, registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG, registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION, FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let recovery = RuntimeCompartmentV1::decode(recovery_frame.body)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    recovery.validate_against_policy(liveness_policy)
        .map_err(|_| FailureAction13MaterialError::ChainAuthority)?;
    let (policy_address, _) = Address::find_program_address(&[SEED_FAILURE_POLICY, &policy.liveness_policy_id.bytes()], &release.program_id);
    let (recovery_address, _) = Address::find_program_address(&[SEED_FAILURE_RECOVERY, &policy.liveness_lifecycle_id.bytes(), &policy.generation.to_le_bytes()], &release.program_id);
    if snapshot.failure_liveness_policy.data.len() != FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1
        || snapshot.failure_recovery_compartment.data.len() != FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1
        || snapshot.failure_liveness_policy.address != policy_address
        || snapshot.failure_recovery_compartment.address != recovery_address
        || recovery.kind != RuntimeCompartmentKindV1::Recovery
        || recovery.phase != RuntimeCompartmentPhaseV1::Active
        || recovery.identity.policy_id.bytes() != policy.liveness_policy_id.bytes()
        || recovery.identity.lifecycle_id.bytes() != policy.liveness_lifecycle_id.bytes()
        || recovery.identity.account_id.bytes() != snapshot.failure_recovery_compartment.address.to_bytes()
        || recovery.identity.owner.bytes() != release.program_id.to_bytes()
        || recovery.identity.generation != policy.generation
        || recovery.quote_schedule_id.bytes() != policy.recovery_quote_schedule_id.bytes()
        || recovery.maximum_calls != quote.maximum_calls
        || recovery.maximum_lamports_per_call != quote.maximum_lamports_per_call().map_err(|_| FailureAction13MaterialError::ChainAuthority)?
        || recovery.capitalized_work_lamports != quote.work_principal_lamports().map_err(|_| FailureAction13MaterialError::ChainAuthority)?
        || snapshot.failure_recovery_compartment.lamports < recovery.expected_account_balance_lamports().map_err(|_| FailureAction13MaterialError::ChainAuthority)?
    {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    Ok(recovery)
}

fn authenticate_release(release: &IndexedProgramRelease) -> Result<()> {
    release.validate().map_err(|_| FailureAction13MaterialError::CheckedRelease)?;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: registry::RECOVERY_FAMILY_TAG,
        family_version: registry::RECOVERY_FAMILY_VERSION,
        local_action: RecoveryAction::CloseIntervalConsensusWork.tag(),
    };
    if !release.families.contains(&CanonicalFamily::Failure)
        || release.enabled_intents.binary_search(&coordinate).is_err()
    {
        return Err(FailureAction13MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_material_release(material: &ChainDerivedFailureAction13MaterialV1, release: &IndexedProgramRelease) -> Result<()> {
    authenticate_release(release)?;
    if release.key() != material.checked_release_key
        || release.program_id != material.program_id
        || release.program_data != material.program_data
        || release.release_manifest_sha256 != material.release_manifest_sha256
        || release.capability_profile_id != material.capability_profile_id
    {
        return Err(FailureAction13MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_provenance(release: &IndexedProgramRelease, accounts: &[&ObservedRpcAccount; FAILURE_ACTION13_ACCOUNT_COUNT_V1]) -> Result<()> {
    let first = &accounts[0].provenance;
    let release_key = release.key();
    if first.commitment != RpcCommitment::Finalized || first.slot == 0 || first.release_key != release_key
        || accounts.iter().any(|account| account.provenance.commitment != RpcCommitment::Finalized || account.provenance.slot != first.slot || account.provenance.cluster_key != first.cluster_key || account.provenance.release_key != release_key)
    {
        return Err(FailureAction13MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn authenticate_lookup_provenance(lookup: &ObservedRpcAccount, first: &ObservedRpcAccount) -> Result<()> {
    if lookup.provenance.commitment != RpcCommitment::Finalized || lookup.provenance.slot != first.provenance.slot || lookup.provenance.cluster_key != first.provenance.cluster_key || lookup.provenance.release_key != first.provenance.release_key {
        return Err(FailureAction13MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn authenticate_role_shapes(release: &IndexedProgramRelease, accounts: &[&ObservedRpcAccount; FAILURE_ACTION13_ACCOUNT_COUNT_V1]) -> Result<()> {
    for (index, account) in accounts.iter().enumerate() {
        if account.address == Address::default() || account.executable != (index == 8) {
            return Err(FailureAction13MaterialError::ChainAuthority);
        }
        if accounts[index + 1..].iter().any(|other| other.address == account.address) {
            return Err(FailureAction13MaterialError::ChainAuthority);
        }
    }
    for index in (0..=7).chain(10..=15) {
        if accounts[index].owner != release.program_id {
            return Err(FailureAction13MaterialError::ChainAuthority);
        }
    }
    Ok(())
}

fn require_artifact(release: &IndexedProgramRelease, account: &ObservedRpcAccount, kind: ArtifactKind, semantic_id: [u8; 32]) -> Result<()> {
    let (address, _) = Address::find_program_address(&[SEED_PRODUCT_ARTIFACT, &[kind.byte()], &semantic_id], &release.program_id);
    if account.address != address || account.owner != release.program_id || account.executable {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    Ok(())
}

fn authenticate_loader_pair(program: &ObservedRpcAccount, program_data: &ObservedRpcAccount, release: RegistryProgramReleaseV2) -> Result<()> {
    const PROGRAM_METADATA_BYTES: usize = 36;
    const PROGRAMDATA_METADATA_BYTES: usize = 45;
    let program_data_sha256: [u8; 32] = Sha256::digest(&program_data.data).into();
    if program.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || program_data.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || !program.executable || program_data.executable
        || program.data.len() < PROGRAM_METADATA_BYTES || program_data.data.len() < PROGRAMDATA_METADATA_BYTES
        || program.data.get(..4) != Some(2_u32.to_le_bytes().as_slice())
        || program_data.data.get(..4) != Some(3_u32.to_le_bytes().as_slice())
        || program.data.get(4..36) != Some(program_data.address.to_bytes().as_slice())
        || release.program.bytes() != program.address.to_bytes()
        || release.programdata.bytes() != program_data.address.to_bytes()
        || release.programdata_sha256.bytes() != program_data_sha256
    {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    let deployment_slot = u64::from_le_bytes(program_data.data[4..12].try_into().map_err(|_| FailureAction13MaterialError::ChainAuthority)?);
    let authority_canonical = match program_data.data[12] {
        0 => program_data.data[13..45].iter().all(|byte| *byte == 0),
        1 => program_data.data[13..45].iter().any(|byte| *byte != 0),
        _ => false,
    };
    if !authority_canonical || release.deployment_slot != deployment_slot || match release.locus { RegistryReleaseLocusV2::SynthesizedGenesisZero => deployment_slot != 0, RegistryReleaseLocusV2::ObservedPositive => deployment_slot == 0 } {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    Ok(())
}

fn require_pda(program_id: Address, observed: Address, bump: u8, seeds: &[&[u8]]) -> Result<()> {
    let (expected, expected_bump) = Address::find_program_address(seeds, &program_id);
    if observed != expected || bump != expected_bump {
        return Err(FailureAction13MaterialError::ChainAuthority);
    }
    Ok(())
}

fn snapshot_digest(accounts: &[&ObservedRpcAccount; FAILURE_ACTION13_ACCOUNT_COUNT_V1]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dragons-clutch/operator/failure-action13-finalized-snapshot/v1");
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

fn map_construction(_: ConstructionError) -> FailureAction13MaterialError {
    FailureAction13MaterialError::Construction
}
