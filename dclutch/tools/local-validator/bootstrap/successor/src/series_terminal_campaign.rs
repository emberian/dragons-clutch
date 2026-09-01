//! Durable, restart-safe execution evidence for terminal recurring-Series acts.
//!
//! The Series lifecycle planner is the only action selector. This module never
//! asks a caller to choose Retire or Close and never reconstructs a Series
//! request. A selected-release adapter supplies the already-authenticated
//! generic Hot V5/ProfileV3 instruction through
//! [`SelectedSeriesPhysicalActionV1`]. In particular, the executable
//! differential oracles in `dclutch_trading_sbf::series::terminal` are not a
//! physical route and cannot satisfy this interface.
//!
//! The module owns the exterior crash boundaries and evidence semantics:
//!
//! * Planned is an in-memory planner result only; it has no external effect and
//!   is safely recomputed after a crash.
//! * Acquired is address-only provenance appended for the current sequence
//!   after canonical V5 acquisition. It carries no privilege, alias, or
//!   physical-order truth and is reread before use.
//! * Prepared is the first durable action-journal boundary. It binds one exact
//!   planner request and finalized snapshot, an authenticated generic-Hot
//!   physical frame, and the same-slot prestate of every projected protocol
//!   account plus fee payer.
//! * Dispatching persists signed bytes before their first send.
//! * Submitted is poll-only and cannot acquire a new blockhash or signature.
//! * Finalized binds the exact landed packet and a same-ledger poststate.
//!
//! Terminal conservation is donation-inclusive: Retire credits the Ticket's
//! complete observed balance to the authenticated lifecycle RentCredit, and
//! Close credits the root's complete observed balance. Failed hostiles must
//! name an exact custom refusal and prove protocol account bytes and lamports
//! rolled back; only a distinct fee payer may lose the recorded transaction
//! fee.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::shadow_v3::ShadowRequestV3;
use dclutch_market_core_codec::CoreState;
use dclutch_market_core_codec::{SeriesCoreRequestV1, SeriesPermitExpiryRequestV1};
use dclutch_market_retirement_v1_operator::{
    MarketRetirementSnapshotV1, build_checkpoint_market_retirement_v1,
};
use dclutch_operator::series_lifecycle_v3::{
    PlannedSeriesActV3, SeriesConsequenceV3, SeriesCurrentOccurrenceV3, SeriesLifecycleReportV3,
    SeriesLifecycleSnapshotV3, SeriesNextActV3, SeriesTerminalTicketV3,
    inspect_series_lifecycle_v3, series_account_key_v3,
};
use dclutch_operator::{
    Finality,
    direct_inline_route_v3::{DirectHotFixedRouteV3, FinalizedRecordRouteV3},
    series_current_acquisition_v5::{
        SeriesConsumeShadowObservationsV5, SeriesCurrentAcquisitionInputV5,
        SeriesSelectedRecordObservationsV5, acquire_current_series_hot_v5,
    },
    series_hot_v3::{
        CheckedSeriesShadowAcceleratorV3, SeriesCurrentHotPlanV5, inspect_current_series_hot_v5,
    },
};
use dclutch_operator::{Observation, series_hot_v3::SeriesSelectedHotReportV5};
use dclutch_series_v3_kernel::{
    TemplateV3,
    replay::{SERIES_STATE_BYTES_V3, SeriesStateV3, TicketStateV3},
    request::{SeriesActionRequestV3, SeriesActionV3},
    terminal::SeriesLifecycleRentSinkV3,
};
use dclutch_trading_sbf::series::{
    account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
    artifacts_v3::{
        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
    expire_funding_artifacts_v5::{
        SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5, SeriesExpireAccountProfileInputV5,
        SeriesExpireChildRequestsV5,
    },
    occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
    prepare_funding_artifacts_v5::{
        SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5, SeriesPrepareAccountProfileInputV5,
    },
    release_v5::{
        SeriesCurrentReleaseInputV5, SeriesOccurrenceAuthorityV5,
        authenticate_series_selected_action_v5, compile_series_release_v5,
        emit_current_series_release_source_v5,
    },
    template_content_id,
};
use dclutch_versioned_message_operator::ObservedAccount;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey, rent::Rent};
use solana_sdk::{
    message::VersionedMessage,
    signature::{Keypair, Signature, Signer as _},
    transaction::VersionedTransaction,
};

use crate::{
    Error, Result,
    aggregate_retirement_exterior::{
        AggregateRetirementTransportV1, run_authenticated_aggregate_retirement_v1,
    },
    aggregate_retirement_journal::{
        AggregateRetirementCampaignInputV1, AggregateRetirementCampaignV1,
        AggregateRetirementConservationReceiptV1, AggregateRetirementInitialAccountV1,
        authenticate_aggregate_retirement_campaign_v1,
        authenticate_aggregate_retirement_conservation_receipt_v1,
        build_aggregate_retirement_campaign_v1,
    },
    campaign::read_keypair_file,
    rpc::{
        FinalizedSignedPacketV1, Rpc, SignedVersionedPacketV1, parse_json_without_duplicate_keys_v1,
    },
    series_lifecycle_campaign::read_authenticated_series_prefix_found_v2,
};

pub(crate) const SERIES_TERMINAL_JOURNAL_SCHEMA_V1: &str =
    "dclutch-owned-loopback-series-terminal-journal-v1";
pub(crate) const SERIES_TERMINAL_ROLLBACK_SCHEMA_V1: &str =
    "dclutch-owned-loopback-series-terminal-rollback-v1";
pub(crate) const SERIES_TERMINAL_CONSERVATION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-series-terminal-conservation-v1";
pub(crate) const SERIES_COMPLETE_LIFECYCLE_SCHEMA_V1: &str =
    "dclutch-owned-loopback-series-complete-lifecycle-v1";
pub(crate) const SERIES_TERMINAL_CAMPAIGN_COMMAND_V1: &str =
    "local-private-validator-series-terminal-campaign-v1";
const SERIES_TERMINAL_CAMPAIGN_INPUT_SCHEMA_V2: &str =
    "dclutch-owned-loopback-series-terminal-campaign-input-v2";
const SERIES_ACQUIRED_ADDRESS_FRAME_SCHEMA_V2: &str =
    "dclutch-owned-loopback-series-acquired-address-frame-v2";

/// CLI paths and the sole execute switch. Semantic selection is intentionally
/// absent: the live lifecycle planner and current V5 release select the act.
#[derive(Clone, Debug)]
struct SeriesTerminalCampaignArgumentsV1 {
    input: PathBuf,
    journal_dir: PathBuf,
    completion: PathBuf,
    fee_payer_keypair: PathBuf,
    execute: bool,
}

/// Addresses for one finalized Registry record and its vacant staging cursor.
/// No caller-authored privilege is accepted anywhere in the campaign input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesFinalizedRecordAddressesV2 {
    raw: String,
    staging: String,
}

/// Address-only common Hot acquisition coordinates. The operator owns the
/// fixed-coordinate order, privileges, owners, widths, and record admission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesHotFixedAddressesV2 {
    market: String,
    root: String,
    manifest: SeriesFinalizedRecordAddressesV2,
    program_set: SeriesFinalizedRecordAddressesV2,
    descriptor: SeriesFinalizedRecordAddressesV2,
    config: SeriesFinalizedRecordAddressesV2,
    account_profile: SeriesFinalizedRecordAddressesV2,
    request_profile: SeriesFinalizedRecordAddressesV2,
    transition: SeriesFinalizedRecordAddressesV2,
    effect: SeriesFinalizedRecordAddressesV2,
    lifecycle: SeriesFinalizedRecordAddressesV2,
    strategy: SeriesFinalizedRecordAddressesV2,
    activation_cache: String,
    core_program: String,
    core_programdata: String,
    trading_program: String,
    trading_programdata: String,
    registry_program: String,
    rent_sysvar: String,
    instructions_sysvar: String,
    product: SeriesFinalizedRecordAddressesV2,
    result_domain: SeriesFinalizedRecordAddressesV2,
    portfolio: SeriesFinalizedRecordAddressesV2,
    linked_basis: SeriesFinalizedRecordAddressesV2,
    capability_seal: String,
}

/// Consume-only address/provenance input. Exact records, deployment, request,
/// caller PDA, and checked-manifest identity are reauthenticated by the
/// production acquisition operator; this carries no privilege or alias truth.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesConsumeShadowAcquisitionV2 {
    certificate: SeriesFinalizedRecordAddressesV2,
    artifact: SeriesFinalizedRecordAddressesV2,
    accelerator_program: String,
    accelerator_programdata: String,
    caller_authority: String,
    checked_manifest_sha256: String,
    request_base64: String,
}

impl SeriesFinalizedRecordAddressesV2 {
    fn addresses(&self) -> [&str; 2] {
        [&self.raw, &self.staging]
    }
}

impl SeriesHotFixedAddressesV2 {
    fn addresses(&self) -> Vec<&str> {
        let mut output = vec![self.market.as_str(), self.root.as_str()];
        for record in [
            &self.manifest,
            &self.program_set,
            &self.descriptor,
            &self.config,
            &self.account_profile,
            &self.request_profile,
            &self.transition,
            &self.effect,
            &self.lifecycle,
            &self.strategy,
        ] {
            output.extend(record.addresses());
        }
        output.extend([
            self.activation_cache.as_str(),
            self.core_program.as_str(),
            self.core_programdata.as_str(),
            self.trading_program.as_str(),
            self.trading_programdata.as_str(),
            self.registry_program.as_str(),
            self.rent_sysvar.as_str(),
            self.instructions_sysvar.as_str(),
        ]);
        for record in [
            &self.product,
            &self.result_domain,
            &self.portfolio,
            &self.linked_basis,
        ] {
            output.extend(record.addresses());
        }
        output.push(self.capability_seal.as_str());
        output
    }
}

impl SeriesConsumeShadowAcquisitionV2 {
    fn addresses(&self) -> [&str; 7] {
        [
            &self.certificate.raw,
            &self.certificate.staging,
            &self.artifact.raw,
            &self.artifact.staging,
            &self.accelerator_program,
            &self.accelerator_programdata,
            &self.caller_authority,
        ]
    }
}

/// Current-occurrence evidence routing. Immutable bodies and replay bytes are
/// read from the same finalized RPC response as the Hot account frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesCurrentOccurrenceRouteV1 {
    occurrence_record: String,
    occurrence_staging: String,
    ticket_record: String,
    ticket_staging: String,
    ticket_replay: Option<String>,
    siblings: Vec<String>,
}

/// Terminal Ticket routing. The planner hostile-decodes both live accounts and
/// proves the replay is terminal before it may select Retire.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesTerminalTicketRouteV1 {
    ticket_record: String,
    ticket_staging: String,
    ticket_replay: String,
}

/// One sequence-indexed acquisition recipe. Future entries are inert candidate
/// addresses, not preauthorized banks: only the current entry can become a
/// durable frame after one finalized observation passes the canonical V5
/// acquisition operator.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesHotAcquisitionRecipeV2 {
    sequence: u32,
    fixed: SeriesHotFixedAddressesV2,
    runtime_logical_accounts: Vec<String>,
    consume_shadow: Option<SeriesConsumeShadowAcquisitionV2>,
    current_occurrence: Option<SeriesCurrentOccurrenceRouteV1>,
    terminal_ticket: Option<SeriesTerminalTicketRouteV1>,
    lifecycle_rent_credit: Option<String>,
    expire_permit: Option<String>,
}

/// One address-only current frame appended after canonical acquisition. It
/// persists no caller-authored privilege, alias, or physical-order truth.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesAcquiredAddressFrameV2 {
    schema: String,
    campaign_sha256: String,
    ledger_identity_sha256: String,
    sequence: u32,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    action: SeriesJournalActionV1,
    request_sha256: String,
    selected_release_set: String,
    recipe: SeriesHotAcquisitionRecipeV2,
    frame_sha256: String,
}

/// Candidate corpus consumed by the current semantic emitters. These values
/// cannot authorize a release: the production operator requires the emitted
/// ProgramSet, descriptor, ProfileV3, lifecycle, strategy, transition, and
/// EffectV5 bytes to match the live finalized accounts byte-for-byte.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesCurrentSourceCorpusV1 {
    consume_shadow_certificate_program: String,
    prepare_fixed_data_lengths: Vec<u32>,
    prepare_ticket_rent_lamports: u64,
    prepare_projected_initialize_base64: String,
    prepare_projected_open_base64: String,
    prepare_replay_initialize_base64: String,
    prepare_escrow_open_base64: String,
    prepare_escrow_lock_base64: String,
    consume_fixed_data_lengths: Vec<u32>,
    consume_lock_base64: String,
    consume_core_base64: String,
    consume_realize_base64: String,
    consume_claims_base64: String,
    consume_funding_count: u32,
    expire_fixed_data_lengths: Vec<u32>,
    expire_refund_base64: String,
    expire_close_vault_base64: String,
    expire_close_replay_base64: String,
    expire_projected_abort_base64: String,
    expire_permit_expiry_base64: String,
    expire_core_base64: String,
}

/// Existing Found transaction evidence. The constructor reauthenticates its
/// campaign/ledger identities before it can enter the final Series ledger.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesFoundInputV1 {
    root: String,
    parent_market: String,
    parent_generation: u64,
    template: String,
    signature: String,
    finalized_slot: u64,
    packet_sha256: String,
    poststate_sha256: String,
}

/// Complete acquisition manifest. RPC and genesis are immutable campaign
/// provenance; all physical semantics are re-derived from live accounts.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesTerminalCampaignInputV2 {
    schema: String,
    rpc_url: String,
    canonical_ledger_path: String,
    genesis_hash: String,
    prefix_ledger: String,
    prefix_ledger_sha256: String,
    fee_payer: String,
    lookup_table: String,
    lookup_table_sha256: String,
    current_source: SeriesCurrentSourceCorpusV1,
    acquisition_recipes: Vec<SeriesHotAcquisitionRecipeV2>,
    market_retirements: Vec<SeriesMarketRetirementRouteV1>,
}

/// Address-only acquisition and durable transport paths for one generic Market
/// retirement. The exact 31 semantic roles are authenticated by the generic
/// retirement operator; Series supplies only their live addresses.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesMarketRetirementRouteV1 {
    ordinal: u32,
    role_addresses: BTreeMap<String, String>,
    snapshot: PathBuf,
    campaign: PathBuf,
    journal_dir: PathBuf,
    completion: PathBuf,
}

/// Content-complete same-finalized snapshot persisted before the first generic
/// retirement mutation, so recovery never tries to reconstruct historical
/// prestate after accounts have closed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableSeriesMarketRetirementSnapshotV1 {
    schema: String,
    campaign_sha256: String,
    ledger_identity_sha256: String,
    ordinal: u32,
    payer: String,
    lookup_table: String,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    role_addresses: BTreeMap<String, String>,
    accounts: BTreeMap<String, DurableSeriesAccountV1>,
    snapshot_sha256: String,
}

const SERIES_MARKET_RETIREMENT_SNAPSHOT_SCHEMA_V1: &str =
    "dclutch-owned-loopback-series-market-retirement-snapshot-v1";
const SERIES_RETIREMENT_ROLES_V1: [&str; 31] = [
    "market",
    "rent-credit",
    "activation-cache",
    "registry-program",
    "core-program",
    "core-programdata",
    "claims-program",
    "claims-programdata",
    "resolution-program",
    "resolution-programdata",
    "custody-program",
    "custody-programdata",
    "rent-program",
    "source-receipt",
    "claims-aggregate",
    "custody-replay",
    "hoard-vault",
    "custody-authority",
    "collateral-mint",
    "collateral-token-program",
    "realm-raw",
    "realm-staging",
    "infrastructure-profile",
    "registry-artifact-raw",
    "registry-artifact-staging",
    "registry-programdata",
    "rent-artifact-raw",
    "rent-artifact-staging",
    "rent-programdata",
    "rent-sysvar",
    "refund-wallet",
];

/// Host-decoded current-source corpus. Fixed arrays are exact-width so no
/// runtime slice can silently alter one emitter's geometry.
struct DecodedSeriesCurrentSourceV1 {
    consume_shadow_certificate_program: ContentId,
    prepare_fixed_data_lengths: [u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
    prepare_ticket_rent_lamports: u64,
    prepare_projected_initialize: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    prepare_projected_open: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    prepare_replay_initialize: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    prepare_escrow_open: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    prepare_escrow_lock: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    consume_fixed_data_lengths: [u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    consume_lock: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    consume_core: [u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3],
    consume_realize: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    consume_claims: [u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3],
    consume_funding_count: u32,
    expire_fixed_data_lengths: [u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
    expire_refund: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    expire_close_vault: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    expire_close_replay: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    expire_projected_abort: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    expire_permit_expiry: SeriesPermitExpiryRequestV1,
    expire_core: SeriesCoreRequestV1,
}

/// One bounded same-finalized RPC acquisition. `accounts` includes every Hot,
/// lifecycle, source-role, routing, and fee-payer key requested by the frame.
struct AcquiredSeriesSelectedV1 {
    observation: Observation,
    accounts: BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    lifecycle: SeriesLifecycleReportV3,
    selected: SeriesSelectedHotReportV5,
}

/// Durable journal phase. `Dispatching` is the fsync-before-send boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SeriesTerminalJournalPhaseV1 {
    Planned,
    Prepared,
    Dispatching,
    Submitted,
    Finalized,
}

/// Planner-derived action label retained only for durable routing and evidence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SeriesJournalActionV1 {
    Prepare,
    Consume,
    Expire,
    Retire,
    Close,
}

impl SeriesJournalActionV1 {
    fn from_kernel(value: SeriesActionV3) -> Self {
        match value {
            SeriesActionV3::Prepare => Self::Prepare,
            SeriesActionV3::Consume => Self::Consume,
            SeriesActionV3::Expire => Self::Expire,
            SeriesActionV3::Retire => Self::Retire,
            SeriesActionV3::Close => Self::Close,
        }
    }

    const fn terminal(self) -> bool {
        matches!(self, Self::Retire | Self::Close)
    }
}

/// Exact generic-Hot mechanism authenticated from selected V5 artifacts.
///
/// This is an evidence label, not a second interpreter. The implementation of
/// [`SelectedSeriesPhysicalActionV1`] must project it from the selected
/// ProfileV3/EffectV5/lifecycle artifacts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SeriesPhysicalMechanismV1 {
    Occurrence,
    RetireFundingV5Close,
    CloseLifecycleOnlyRoot,
}

/// One existing local-validator ledger. The mutable directory is identified by
/// its canonical path and immutable genesis hash, never by a fresh campaign id.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesLedgerIdentityV1 {
    pub(crate) canonical_ledger_path: String,
    pub(crate) genesis_hash: String,
    pub(crate) identity_sha256: String,
}

impl SeriesLedgerIdentityV1 {
    pub(crate) fn admit(canonical_ledger_path: String, genesis_hash: String) -> Result<Self> {
        if !canonical_ledger_path.starts_with('/') || genesis_hash.is_empty() {
            return Err(refusal(
                "validator ledger identity requires an absolute path and genesis hash",
            ));
        }
        let mut value = Self {
            canonical_ledger_path,
            genesis_hash,
            identity_sha256: String::new(),
        };
        value.identity_sha256 = ledger_identity_digest_v1(&value)?;
        authenticate_ledger_identity_v1(&value)?;
        Ok(value)
    }
}

/// Same-finalized observation which was passed to the lifecycle planner.
#[derive(Clone, Debug)]
pub(crate) struct SeriesPlannerObservationV1 {
    pub(crate) campaign_sha256: String,
    pub(crate) ledger: SeriesLedgerIdentityV1,
    pub(crate) finalized_slot: u64,
    /// Digest of the complete hostile-decoded RPC snapshot, including Clock.
    pub(crate) snapshot_sha256: String,
}

/// Exact selected-artifact identities behind one physical generic-Hot frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesSelectedAuthorityIdsV1 {
    pub(crate) release_set: [u8; 32],
    pub(crate) program_set_v2: [u8; 32],
    /// Selected CapabilityProgramV4 content identity.
    pub(crate) descriptor: [u8; 32],
    /// Selected AccountProfileV3 content identity.
    pub(crate) profile_v3: [u8; 32],
    /// Selected RequestProfileV1 content identity.
    pub(crate) request_profile: [u8; 32],
    /// Selected StateLifecyclePolicyV5 content identity.
    pub(crate) lifecycle_policy: [u8; 32],
    /// Selected ExecutionStrategyV2 content identity.
    pub(crate) strategy: [u8; 32],
    /// Selected TransitionV3 content identity.
    pub(crate) transition: [u8; 32],
    /// Selected EffectV5 content identity.
    pub(crate) effect_v5: [u8; 32],
}

/// Action-specific account roles authenticated by the selected release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesPhysicalRoleKeysV1 {
    pub(crate) root: Pubkey,
    pub(crate) ticket: Option<Pubkey>,
    pub(crate) rent_credit: Option<Pubkey>,
    /// Parent Market which owns the recurring Series capability root.
    pub(crate) parent_market: Pubkey,
    /// Exact parent Market generation authenticated by the Hot envelope/root.
    pub(crate) parent_market_generation: u64,
    /// Canonical child/future Market selected for an occurrence action.
    pub(crate) occurrence_market: Option<Pubkey>,
    /// Exact generation of the occurrence Market projection.
    pub(crate) occurrence_market_generation: Option<u64>,
    /// Canonical pre-Market permit, present only for occurrence actions.
    pub(crate) occurrence_permit: Option<Pubkey>,
}

/// Narrow integration seam for the forthcoming selected V5/ProfileV3 API.
///
/// Only an implementation backed by that API belongs in production. The
/// producer checks the returned request byte-for-byte against the planner and
/// binds the returned top-level Trading instruction. A Series-family raw
/// terminal/oracle instruction must never implement this trait.
pub(crate) trait SelectedSeriesPhysicalActionV1 {
    /// Planner request independently reauthenticated by the selected API.
    fn canonical_request_bytes(&self) -> &[u8];
    /// Action decoded by the selected API.
    fn action(&self) -> SeriesActionV3;
    /// Same finalized observation carried by the selected physical report.
    fn observation(&self) -> Observation;
    /// Current Trading program selected by the release.
    fn trading_program(&self) -> Pubkey;
    /// Exact top-level generic Hot instruction.
    fn generic_hot_instruction(&self) -> Instruction;
    /// Selected release and all five action-artifact identities.
    fn selected_authority_ids(&self) -> SeriesSelectedAuthorityIdsV1;
    /// Authenticated action-specific physical roles.
    fn role_keys(&self) -> SeriesPhysicalRoleKeysV1;
    /// Mechanism proven by the selected EffectV5 and lifecycle policy.
    fn mechanism(&self) -> SeriesPhysicalMechanismV1;
    /// Lifecycle consequence selected by the sole planner.
    fn consequence(&self) -> SeriesConsequenceV3;
}

/// Production adapter from the canonical current-source V5 operator report.
/// The report is already the semantic owner of selection, account bindings,
/// runtime keys, release identities, and the unsigned generic-Hot frame; this
/// implementation only projects those checked facts into durable exterior
/// evidence.
impl SelectedSeriesPhysicalActionV1 for SeriesSelectedHotReportV5 {
    fn canonical_request_bytes(&self) -> &[u8] {
        &self.selected.request_bytes
    }

    fn action(&self) -> SeriesActionV3 {
        self.selected.action
    }

    fn observation(&self) -> Observation {
        self.observation
    }

    fn trading_program(&self) -> Pubkey {
        self.trading_program
    }

    fn generic_hot_instruction(&self) -> Instruction {
        self.instruction.clone()
    }

    fn selected_authority_ids(&self) -> SeriesSelectedAuthorityIdsV1 {
        SeriesSelectedAuthorityIdsV1 {
            release_set: self.release_set,
            program_set_v2: self.program_set_id,
            descriptor: hash(&self.selected.descriptor).to_bytes(),
            profile_v3: self.selected.artifact_ids.account_profile,
            request_profile: self.selected.artifact_ids.request_profile,
            lifecycle_policy: self.selected.artifact_ids.lifecycle,
            strategy: self.selected.artifact_ids.strategy,
            transition: self.selected.artifact_ids.transition,
            effect_v5: self.selected.artifact_ids.effect,
        }
    }

    fn role_keys(&self) -> SeriesPhysicalRoleKeysV1 {
        SeriesPhysicalRoleKeysV1 {
            root: self.roles.root,
            ticket: self.roles.ticket,
            rent_credit: self.roles.rent_credit,
            parent_market: self.parent_market,
            parent_market_generation: self.parent_generation,
            occurrence_market: self.roles.occurrence_market,
            occurrence_market_generation: self.roles.occurrence_generation,
            occurrence_permit: self.roles.permit,
        }
    }

    fn mechanism(&self) -> SeriesPhysicalMechanismV1 {
        match self.selected.action {
            SeriesActionV3::Prepare | SeriesActionV3::Consume | SeriesActionV3::Expire => {
                SeriesPhysicalMechanismV1::Occurrence
            }
            SeriesActionV3::Retire => SeriesPhysicalMechanismV1::RetireFundingV5Close,
            SeriesActionV3::Close => SeriesPhysicalMechanismV1::CloseLifecycleOnlyRoot,
        }
    }

    fn consequence(&self) -> SeriesConsequenceV3 {
        self.consequence
    }
}

impl DecodedSeriesCurrentSourceV1 {
    fn decode(candidate: &SeriesCurrentSourceCorpusV1) -> Result<Self> {
        let consume_shadow_certificate_program = ContentId::new(parse_hex32_v1(
            &candidate.consume_shadow_certificate_program,
            "Series Consume Shadow certificate program",
        )?)
        .map_err(|_| refusal("Series Consume Shadow certificate program was zero"))?;
        let prepare_fixed_data_lengths = candidate
            .prepare_fixed_data_lengths
            .clone()
            .try_into()
            .map_err(|_| refusal("Series Prepare fixed-width corpus changed cardinality"))?;
        let consume_fixed_data_lengths = candidate
            .consume_fixed_data_lengths
            .clone()
            .try_into()
            .map_err(|_| refusal("Series Consume fixed-width corpus changed cardinality"))?;
        let expire_fixed_data_lengths = candidate
            .expire_fixed_data_lengths
            .clone()
            .try_into()
            .map_err(|_| refusal("Series Expire fixed-width corpus changed cardinality"))?;
        let expire_permit_expiry = SeriesPermitExpiryRequestV1::decode(&decode_base64(
            &candidate.expire_permit_expiry_base64,
            "Series Expire permit request",
        )?)
        .map_err(|_| refusal("Series Expire permit request was not canonical"))?;
        let expire_core = SeriesCoreRequestV1::decode(&decode_base64(
            &candidate.expire_core_base64,
            "Series Expire Core request",
        )?)
        .map_err(|_| refusal("Series Expire Core request was not canonical"))?;
        if candidate.consume_funding_count == 0 || candidate.prepare_ticket_rent_lamports == 0 {
            return Err(refusal(
                "Series Consume funding span or Prepare Ticket rent was zero",
            ));
        }
        Ok(Self {
            consume_shadow_certificate_program,
            prepare_fixed_data_lengths,
            prepare_ticket_rent_lamports: candidate.prepare_ticket_rent_lamports,
            prepare_projected_initialize: decode_exact_base64_v1(
                &candidate.prepare_projected_initialize_base64,
                "Series Prepare projected initialize",
            )?,
            prepare_projected_open: decode_exact_base64_v1(
                &candidate.prepare_projected_open_base64,
                "Series Prepare projected open",
            )?,
            prepare_replay_initialize: decode_exact_base64_v1(
                &candidate.prepare_replay_initialize_base64,
                "Series Prepare replay initialize",
            )?,
            prepare_escrow_open: decode_exact_base64_v1(
                &candidate.prepare_escrow_open_base64,
                "Series Prepare escrow open",
            )?,
            prepare_escrow_lock: decode_exact_base64_v1(
                &candidate.prepare_escrow_lock_base64,
                "Series Prepare escrow lock",
            )?,
            consume_fixed_data_lengths,
            consume_lock: decode_exact_base64_v1(
                &candidate.consume_lock_base64,
                "Series Consume lock",
            )?,
            consume_core: decode_exact_base64_v1(
                &candidate.consume_core_base64,
                "Series Consume Core",
            )?,
            consume_realize: decode_exact_base64_v1(
                &candidate.consume_realize_base64,
                "Series Consume realize",
            )?,
            consume_claims: decode_exact_base64_v1(
                &candidate.consume_claims_base64,
                "Series Consume Claims",
            )?,
            consume_funding_count: candidate.consume_funding_count,
            expire_fixed_data_lengths,
            expire_refund: decode_exact_base64_v1(
                &candidate.expire_refund_base64,
                "Series Expire refund",
            )?,
            expire_close_vault: decode_exact_base64_v1(
                &candidate.expire_close_vault_base64,
                "Series Expire close vault",
            )?,
            expire_close_replay: decode_exact_base64_v1(
                &candidate.expire_close_replay_base64,
                "Series Expire close replay",
            )?,
            expire_projected_abort: decode_exact_base64_v1(
                &candidate.expire_projected_abort_base64,
                "Series Expire projected abort",
            )?,
            expire_permit_expiry,
            expire_core,
        })
    }

    fn input(&self, template: ContentId) -> SeriesCurrentReleaseInputV5<'_> {
        SeriesCurrentReleaseInputV5 {
            template,
            consume_shadow_certificate_program: self.consume_shadow_certificate_program,
            prepare_profile: SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &self.prepare_fixed_data_lengths,
            },
            prepare_requests: SeriesPrepareChildRequestsV4 {
                projected_initialize: &self.prepare_projected_initialize,
                projected_open: &self.prepare_projected_open,
                replay_initialize: &self.prepare_replay_initialize,
                escrow_open: &self.prepare_escrow_open,
                escrow_lock: &self.prepare_escrow_lock,
            },
            prepare_ticket_rent_lamports: self.prepare_ticket_rent_lamports,
            consume_observed_data_lengths: &self.consume_fixed_data_lengths,
            consume_requests: SeriesConsumeChildRequestsV4 {
                lock: &self.consume_lock,
                core: &self.consume_core,
                realize: &self.consume_realize,
                claims: &self.consume_claims,
            },
            consume_funding_count: self.consume_funding_count,
            expire_profile: SeriesExpireAccountProfileInputV5 {
                fixed_data_lengths: &self.expire_fixed_data_lengths,
            },
            expire_requests: SeriesExpireChildRequestsV5 {
                refund: &self.expire_refund,
                close_vault: &self.expire_close_vault,
                close_replay: &self.expire_close_replay,
                projected_abort: &self.expire_projected_abort,
                permit_expiry: self.expire_permit_expiry,
                core_expire: self.expire_core,
            },
        }
    }
}

fn acquire_current_series_selected_v1(
    rpc: &mut Rpc,
    frame: &SeriesHotAcquisitionRecipeV2,
    source: &DecodedSeriesCurrentSourceV1,
    payer: Pubkey,
    lookup_table: Pubkey,
) -> Result<AcquiredSeriesSelectedV1> {
    let mut keys = BTreeSet::new();
    for address in frame
        .fixed
        .addresses()
        .into_iter()
        .chain(frame.runtime_logical_accounts.iter().map(String::as_str))
        .chain(
            frame
                .consume_shadow
                .iter()
                .flat_map(SeriesConsumeShadowAcquisitionV2::addresses),
        )
    {
        keys.insert(parse_pubkey(address, "Series acquisition account")?);
    }
    if let Some(current) = &frame.current_occurrence {
        for (address, label) in [
            (&current.occurrence_record, "Series occurrence record"),
            (&current.occurrence_staging, "Series occurrence staging"),
            (&current.ticket_record, "Series Ticket record"),
            (&current.ticket_staging, "Series Ticket staging"),
        ] {
            keys.insert(parse_pubkey(address, label)?);
        }
        if let Some(replay) = &current.ticket_replay {
            keys.insert(parse_pubkey(replay, "Series current Ticket replay")?);
        }
    }
    if let Some(terminal) = &frame.terminal_ticket {
        for (address, label) in [
            (&terminal.ticket_record, "Series terminal Ticket record"),
            (&terminal.ticket_staging, "Series terminal Ticket staging"),
            (&terminal.ticket_replay, "Series terminal Ticket replay"),
        ] {
            keys.insert(parse_pubkey(address, label)?);
        }
    }
    for candidate in [
        frame.lifecycle_rent_credit.as_ref(),
        frame.expire_permit.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        keys.insert(parse_pubkey(
            candidate,
            "Series lifecycle acquisition role",
        )?);
    }
    keys.insert(payer);
    keys.insert(lookup_table);
    if keys.is_empty() || keys.len() > 512 {
        return Err(refusal(
            "Series acquisition account set was empty or exceeded 512",
        ));
    }
    let keys = keys.into_iter().collect::<Vec<_>>();
    let (slot, values) = rpc.finalized_accounts(&keys, 0)?;
    if slot == 0 || values.len() != keys.len() {
        return Err(refusal(
            "Series acquisition was not one complete finalized account vector",
        ));
    }
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    let accounts = keys
        .into_iter()
        .zip(values)
        .map(|(key, value)| {
            (
                key,
                value.map(|account| SeriesObservedAccountV1 {
                    key,
                    owner: account.owner,
                    lamports: account.lamports,
                    executable: account.executable,
                    data: account.data,
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let fixed = series_hot_fixed_route_v2(&frame.fixed, &accounts, observation)?;
    let template_bytes = fixed.config.raw.data.clone();
    let template = TemplateV3::decode(&template_bytes)
        .map_err(|_| refusal("Series live Template refused hostile decode"))?;
    let template_id = template_content_id(&template_bytes)
        .map_err(|_| refusal("Series live Template identity refused"))?;
    let root_tail = fixed
        .root
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or_else(|| refusal("Series composite root omitted its replay tail"))?;
    if root_tail.len() != SERIES_STATE_BYTES_V3 {
        return Err(refusal("Series composite root replay tail changed width"));
    }
    let series = SeriesStateV3::decode(root_tail, template.occurrence_count())
        .map_err(|_| refusal("Series live replay tail refused hostile decode"))?;
    let root_lamports = fixed.root.lamports;
    let root_data_len = fixed.root.data.len();
    let rent: Rent = bincode::deserialize(&fixed.rent_sysvar.data)
        .map_err(|_| refusal("Series same-slot Rent sysvar refused decode"))?;
    let siblings = frame
        .current_occurrence
        .as_ref()
        .map(|current| {
            current
                .siblings
                .iter()
                .map(|value| parse_hex32_v1(value, "Series occurrence sibling"))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();
    let current = frame
        .current_occurrence
        .as_ref()
        .map(|route| -> Result<SeriesCurrentOccurrenceV3<'_>> {
            let occurrence = required_series_account_v1(
                &accounts,
                parse_pubkey(&route.occurrence_record, "Series occurrence record")?,
                "Series occurrence record",
            )?;
            let ticket = required_series_account_v1(
                &accounts,
                parse_pubkey(&route.ticket_record, "Series Ticket record")?,
                "Series Ticket record",
            )?;
            let ticket_state = route
                .ticket_replay
                .as_ref()
                .map(|key| -> Result<Option<TicketStateV3>> {
                    let key = parse_pubkey(key, "Series current Ticket replay")?;
                    accounts
                        .get(&key)
                        .ok_or_else(|| refusal("Series current Ticket replay was not observed"))?
                        .as_ref()
                        .map(|account| {
                            TicketStateV3::decode(&account.data).map_err(|_| {
                                refusal("Series current Ticket replay refused hostile decode")
                            })
                        })
                        .transpose()
                })
                .transpose()?
                .flatten();
            Ok(SeriesCurrentOccurrenceV3 {
                occurrence_bytes: &occurrence.data,
                ticket_bytes: &ticket.data,
                siblings: &siblings,
                ticket_state,
            })
        })
        .transpose()?;
    let terminal_ticket = frame
        .terminal_ticket
        .as_ref()
        .map(|route| -> Result<SeriesTerminalTicketV3<'_>> {
            let ticket = required_series_account_v1(
                &accounts,
                parse_pubkey(&route.ticket_record, "Series terminal Ticket record")?,
                "Series terminal Ticket record",
            )?;
            let replay = required_series_account_v1(
                &accounts,
                parse_pubkey(&route.ticket_replay, "Series terminal Ticket replay")?,
                "Series terminal Ticket replay",
            )?;
            let ticket_state = TicketStateV3::decode(&replay.data)
                .map_err(|_| refusal("Series terminal Ticket replay refused hostile decode"))?;
            Ok(SeriesTerminalTicketV3 {
                ticket_bytes: &ticket.data,
                ticket_state,
                observed_lamports: replay.lamports,
                exact_rent: rent.minimum_balance(replay.data.len()),
            })
        })
        .transpose()?;
    let rent_sink = frame
        .lifecycle_rent_credit
        .as_ref()
        .map(|key| -> Result<SeriesLifecycleRentSinkV3> {
            let credit_key = parse_pubkey(key, "Series lifecycle RentCredit")?;
            let credit =
                required_series_account_v1(&accounts, credit_key, "Series lifecycle RentCredit")?;
            let header = CapabilityRootHeaderV1::decode(
                fixed
                    .root
                    .data
                    .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
                    .ok_or_else(|| refusal("Series root omitted its immutable header"))?,
            )
            .map_err(|_| refusal("Series root header refused hostile decode"))?;
            SeriesLifecycleRentSinkV3::admit(
                series_account_key_v3(credit_key.to_bytes())
                    .map_err(|_| refusal("Series RentCredit key refused"))?,
                &credit.data,
                series_account_key_v3(header.market())
                    .map_err(|_| refusal("Series parent Market key refused"))?,
                header.release_set(),
                header.generation(),
                template.refund_owner(),
            )
            .map_err(|_| refusal("Series lifecycle RentCredit refused root/Template binding"))
        })
        .transpose()?;
    let lifecycle_snapshot = SeriesLifecycleSnapshotV3 {
        template_bytes: &template_bytes,
        series,
        now_slot: observation.slot,
        current,
        terminal_ticket,
        observed_root_lamports: root_lamports,
        exact_root_rent: rent.minimum_balance(root_data_len),
        rent_sink,
    };
    let lifecycle = inspect_series_lifecycle_v3(lifecycle_snapshot)
        .map_err(|error| refusal(format!("Series lifecycle planner: {error:?}")))?;
    if source.prepare_ticket_rent_lamports
        != rent.minimum_balance(dclutch_series_v3_kernel::replay::SERIES_TICKET_STATE_BYTES_V3)
    {
        return Err(refusal(
            "Series current-source Ticket rent differed from same-slot Rent",
        ));
    }
    let current_source = source.input(template_id);
    let planned = match lifecycle.next() {
        SeriesNextActV3::Ready(planned) => planned,
        SeriesNextActV3::Acquire(needed) => {
            return Err(refusal(format!(
                "Series lifecycle needs another authenticated acquisition: {needed:?}"
            )));
        }
        SeriesNextActV3::WaitUntil { scheduled_slot } => {
            return Err(refusal(format!(
                "Series lifecycle waits until slot {scheduled_slot}"
            )));
        }
    };
    let owned_release = emit_current_series_release_source_v5(current_source)
        .map_err(|error| refusal(format!("emit current Series V5 release: {error:?}")))?;
    let release = compile_series_release_v5(owned_release.as_source())
        .map_err(|error| refusal(format!("compile current Series V5 release: {error:?}")))?;
    let preselected = authenticate_series_selected_action_v5(
        &release,
        owned_release.as_source(),
        planned.request().as_bytes(),
    )
    .map_err(|error| refusal(format!("authenticate current Series V5 action: {error:?}")))?;
    if preselected.action != planned.action() {
        return Err(refusal(
            "current Series release changed the lifecycle-selected action",
        ));
    }
    let runtime_logical_accounts = frame
        .runtime_logical_accounts
        .iter()
        .map(|address| {
            series_operator_account_from_address_v2(
                &accounts,
                address,
                "Series logical runtime account",
                observation,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let occurrence_record = frame
        .current_occurrence
        .as_ref()
        .map(|route| {
            series_finalized_record_route_v2(
                &accounts,
                &route.occurrence_record,
                &route.occurrence_staging,
                "Series occurrence",
                observation,
            )
        })
        .transpose()?;
    if frame.current_occurrence.is_some() && frame.terminal_ticket.is_some() {
        return Err(refusal(
            "Series acquisition supplied current and terminal Ticket routes together",
        ));
    }
    let ticket_record = match (&frame.current_occurrence, &frame.terminal_ticket) {
        (Some(route), None) => Some(series_finalized_record_route_v2(
            &accounts,
            &route.ticket_record,
            &route.ticket_staging,
            "Series current Ticket",
            observation,
        )?),
        (None, Some(route)) => Some(series_finalized_record_route_v2(
            &accounts,
            &route.ticket_record,
            &route.ticket_staging,
            "Series terminal Ticket",
            observation,
        )?),
        (None, None) => None,
        (Some(_), Some(_)) => unreachable!("refused above"),
    };
    let rent_credit = frame
        .lifecycle_rent_credit
        .as_ref()
        .map(|address| {
            series_operator_account_from_address_v2(
                &accounts,
                address,
                "Series lifecycle RentCredit",
                observation,
            )
        })
        .transpose()?;
    let expire_permit = frame
        .expire_permit
        .as_ref()
        .map(|address| {
            series_operator_account_from_address_v2(
                &accounts,
                address,
                "Series Expire permit",
                observation,
            )
        })
        .transpose()?;
    let shadow_request_bytes = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| decode_base64(&shadow.request_base64, "Series Consume Shadow request"))
        .transpose()?;
    let shadow_request = shadow_request_bytes
        .as_deref()
        .map(ShadowRequestV3::decode)
        .transpose()
        .map_err(|_| refusal("Series Consume Shadow request refused hostile decode"))?;
    let shadow_certificate = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_finalized_record_route_v2(
                &accounts,
                &shadow.certificate.raw,
                &shadow.certificate.staging,
                "Series Consume Shadow certificate",
                observation,
            )
        })
        .transpose()?;
    let shadow_artifact = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_finalized_record_route_v2(
                &accounts,
                &shadow.artifact.raw,
                &shadow.artifact.staging,
                "Series Consume Shadow artifact",
                observation,
            )
        })
        .transpose()?;
    let shadow_accelerator_program = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_operator_account_from_address_v2(
                &accounts,
                &shadow.accelerator_program,
                "Series Consume accelerator program",
                observation,
            )
        })
        .transpose()?;
    let shadow_accelerator_programdata = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_operator_account_from_address_v2(
                &accounts,
                &shadow.accelerator_programdata,
                "Series Consume accelerator ProgramData",
                observation,
            )
        })
        .transpose()?;
    let shadow_caller_authority = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_operator_account_from_address_v2(
                &accounts,
                &shadow.caller_authority,
                "Series Consume Shadow caller authority",
                observation,
            )
        })
        .transpose()?;
    let shadow_checked = match (&frame.consume_shadow, &shadow_artifact) {
        (Some(shadow), Some(artifact)) => Some(CheckedSeriesShadowAcceleratorV3 {
            artifact_release: hash(&artifact.raw.data).to_bytes(),
            accelerator_program: shadow_accelerator_program
                .as_ref()
                .ok_or_else(|| refusal("Series Consume accelerator program was absent"))?
                .key,
            accelerator_programdata: shadow_accelerator_programdata
                .as_ref()
                .ok_or_else(|| refusal("Series Consume accelerator ProgramData was absent"))?
                .key,
            checked_manifest_digest: parse_hex32_v1(
                &shadow.checked_manifest_sha256,
                "Series Consume checked manifest",
            )?,
        }),
        (None, None) => None,
        _ => return Err(refusal("Series Consume Shadow acquisition was incomplete")),
    };
    let shadow = match frame.consume_shadow.as_ref() {
        Some(_) => Some(SeriesConsumeShadowObservationsV5 {
            certificate: shadow_certificate
                .as_ref()
                .ok_or_else(|| refusal("Series Consume Shadow certificate was absent"))?,
            artifact: shadow_artifact
                .as_ref()
                .ok_or_else(|| refusal("Series Consume Shadow artifact was absent"))?,
            accelerator_program: shadow_accelerator_program
                .as_ref()
                .ok_or_else(|| refusal("Series Consume accelerator program was absent"))?,
            accelerator_programdata: shadow_accelerator_programdata
                .as_ref()
                .ok_or_else(|| refusal("Series Consume accelerator ProgramData was absent"))?,
            caller_authority: shadow_caller_authority
                .as_ref()
                .ok_or_else(|| refusal("Series Consume Shadow caller authority was absent"))?,
            checked: shadow_checked
                .ok_or_else(|| refusal("Series Consume checked release was absent"))?,
            request: shadow_request
                .ok_or_else(|| refusal("Series Consume Shadow request was absent"))?,
        }),
        None => None,
    };
    let acquired = acquire_current_series_hot_v5(
        &preselected,
        owned_release.action_artifacts(preselected.action),
        SeriesCurrentAcquisitionInputV5 {
            fixed: &fixed,
            runtime_logical_accounts: &runtime_logical_accounts,
            records: SeriesSelectedRecordObservationsV5 {
                occurrence: occurrence_record.as_ref(),
                ticket: ticket_record.as_ref(),
                rent_credit: rent_credit.as_ref(),
                expire_permit: expire_permit.as_ref(),
            },
            shadow,
            lifecycle: lifecycle_snapshot,
        },
    )
    .map_err(|error| refusal(format!("acquire current Series V5 bank: {error:?}")))?;
    let selected = match inspect_current_series_hot_v5(&acquired.state, current_source)
        .map_err(|error| refusal(format!("current Series V5 operator: {error:?}")))?
    {
        SeriesCurrentHotPlanV5::Ready(report) => report,
        SeriesCurrentHotPlanV5::Acquire(needed) => {
            return Err(refusal(format!(
                "Series lifecycle needs another authenticated acquisition: {needed:?}"
            )));
        }
        SeriesCurrentHotPlanV5::WaitUntil { scheduled_slot } => {
            return Err(refusal(format!(
                "Series lifecycle waits until slot {scheduled_slot}"
            )));
        }
    };
    if selected.observation != observation || selected.selected != preselected {
        return Err(refusal(
            "Series selected report changed the acquired observation or release action",
        ));
    }
    Ok(AcquiredSeriesSelectedV1 {
        observation,
        accounts,
        lifecycle,
        selected,
    })
}

fn series_operator_account_from_address_v2(
    accounts: &BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    address: &str,
    label: &str,
    observation: Observation,
) -> Result<ObservedAccount> {
    let key = parse_pubkey(address, label)?;
    operator_account_v1(
        required_series_account_v1(accounts, key, label)?,
        observation,
    )
}

fn series_finalized_record_route_v2(
    accounts: &BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    raw: &str,
    staging: &str,
    label: &str,
    observation: Observation,
) -> Result<FinalizedRecordRouteV3> {
    Ok(FinalizedRecordRouteV3 {
        raw: series_operator_account_from_address_v2(
            accounts,
            raw,
            &format!("{label} raw"),
            observation,
        )?,
        staging: series_operator_account_from_address_v2(
            accounts,
            staging,
            &format!("{label} staging"),
            observation,
        )?,
    })
}

fn series_hot_fixed_route_v2(
    addresses: &SeriesHotFixedAddressesV2,
    accounts: &BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    observation: Observation,
) -> Result<DirectHotFixedRouteV3> {
    let account = |address: &str, label: &str| {
        series_operator_account_from_address_v2(accounts, address, label, observation)
    };
    let record = |addresses: &SeriesFinalizedRecordAddressesV2, label: &str| {
        series_finalized_record_route_v2(
            accounts,
            &addresses.raw,
            &addresses.staging,
            label,
            observation,
        )
    };
    Ok(DirectHotFixedRouteV3 {
        market: account(&addresses.market, "Series controller Market")?,
        root: account(&addresses.root, "Series capability root")?,
        manifest: record(&addresses.manifest, "Series CapabilityManifest")?,
        program_set: record(&addresses.program_set, "Series CapabilityProgramSet")?,
        descriptor: record(&addresses.descriptor, "Series CapabilityProgram")?,
        config: record(&addresses.config, "Series Template")?,
        account_profile: record(&addresses.account_profile, "Series AccountProfile")?,
        request_profile: record(&addresses.request_profile, "Series RequestProfile")?,
        transition: record(&addresses.transition, "Series Transition")?,
        effect: record(&addresses.effect, "Series Effect")?,
        lifecycle: record(&addresses.lifecycle, "Series lifecycle policy")?,
        strategy: record(&addresses.strategy, "Series execution strategy")?,
        activation_cache: account(&addresses.activation_cache, "Series activation cache")?,
        core_program: account(&addresses.core_program, "Series Core program")?,
        core_programdata: account(&addresses.core_programdata, "Series Core ProgramData")?,
        trading_program: account(&addresses.trading_program, "Series Trading program")?,
        trading_programdata: account(&addresses.trading_programdata, "Series Trading ProgramData")?,
        registry_program: account(&addresses.registry_program, "Series Registry program")?,
        rent_sysvar: account(&addresses.rent_sysvar, "Series Rent sysvar")?,
        instructions_sysvar: account(&addresses.instructions_sysvar, "Series Instructions sysvar")?,
        product: record(&addresses.product, "Series Product")?,
        result_domain: record(&addresses.result_domain, "Series result domain")?,
        portfolio: record(&addresses.portfolio, "Series portfolio")?,
        linked_basis: record(&addresses.linked_basis, "Series linked basis")?,
        capability_seal: account(&addresses.capability_seal, "Series capability seal")?,
    })
}

fn operator_account_v1(
    value: &SeriesObservedAccountV1,
    observation: Observation,
) -> Result<ObservedAccount> {
    if value.key == Pubkey::default() {
        return Err(refusal("Series observed account had the zero key"));
    }
    Ok(ObservedAccount {
        observation,
        key: value.key,
        owner: value.owner,
        lamports: value.lamports,
        executable: value.executable,
        data: value.data.clone(),
    })
}

fn required_series_account_v1<'a>(
    accounts: &'a BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    key: Pubkey,
    label: &str,
) -> Result<&'a SeriesObservedAccountV1> {
    accounts
        .get(&key)
        .ok_or_else(|| refusal(format!("{label} was outside the bounded acquisition")))?
        .as_ref()
        .ok_or_else(|| refusal(format!("{label} was absent")))
}

fn decode_exact_base64_v1<const N: usize>(value: &str, label: &str) -> Result<[u8; N]> {
    decode_base64(value, label)?
        .try_into()
        .map_err(|_| refusal(format!("{label} changed exact width")))
}

fn parse_hex32_v1(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(refusal(format!("{label} was not canonical SHA-256 hex")));
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair =
            std::str::from_utf8(pair).map_err(|_| refusal(format!("{label} was not UTF-8 hex")))?;
        output[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| refusal(format!("{label} was not canonical hex")))?;
    }
    Ok(output)
}

/// Drive one bounded crash-safe pass of the current-source Series campaign.
/// Repeated invocations converge each selected action; no action selector is
/// accepted. `--execute` is the sole boundary that opens the fee-payer key.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_series_terminal_arguments_v1(arguments)?;
    let (input, campaign_sha256) = read_series_terminal_campaign_input_v1(&arguments.input)?;
    authenticate_series_terminal_campaign_input_v1(&input, &arguments, &campaign_sha256)?;
    let prefix_found = read_authenticated_series_prefix_found_v2(
        Path::new(&input.prefix_ledger),
        &input.prefix_ledger_sha256,
    )?;
    let found = SeriesFoundInputV1 {
        root: prefix_found.root,
        parent_market: prefix_found.parent_market,
        parent_generation: prefix_found.parent_generation,
        template: prefix_found.template,
        signature: prefix_found.signature,
        finalized_slot: prefix_found.finalized_slot,
        packet_sha256: prefix_found.packet_sha256,
        poststate_sha256: prefix_found.poststate_sha256,
    };
    let ledger_path = fs::canonicalize(&input.canonical_ledger_path).map_err(|error| {
        Error::new(format!(
            "canonicalize Series validator ledger {}: {error}",
            input.canonical_ledger_path
        ))
    })?;
    if !ledger_path.is_dir() || ledger_path.to_string_lossy() != input.canonical_ledger_path {
        return Err(refusal(
            "Series campaign ledger path was not its exact canonical directory",
        ));
    }
    let ledger = SeriesLedgerIdentityV1::admit(
        input.canonical_ledger_path.clone(),
        input.genesis_hash.clone(),
    )?;
    let payer = parse_pubkey(&input.fee_payer, "Series fee payer")?;
    let lookup_table = parse_pubkey(&input.lookup_table, "Series lookup table")?;
    let source = DecodedSeriesCurrentSourceV1::decode(&input.current_source)?;
    let journals = load_series_terminal_journals_v1(
        &arguments.journal_dir,
        input.acquisition_recipes.len(),
        &campaign_sha256,
        &ledger,
    )?;
    if journals
        .last()
        .is_some_and(|journal| journal.phase != SeriesTerminalJournalPhaseV1::Finalized)
        && journals
            .iter()
            .rev()
            .skip(1)
            .any(|journal| journal.phase != SeriesTerminalJournalPhaseV1::Finalized)
    {
        return Err(refusal(
            "Series durable action set contained more than one active journal",
        ));
    }
    let sequence = journals
        .last()
        .filter(|journal| journal.phase != SeriesTerminalJournalPhaseV1::Finalized)
        .map(|journal| journal.sequence)
        .unwrap_or_else(|| u32::try_from(journals.len()).unwrap_or(u32::MAX));
    authenticate_series_acquired_address_frame_history_v2(
        &arguments.journal_dir,
        &input.acquisition_recipes,
        &journals,
        sequence,
        &campaign_sha256,
        &ledger,
    )?;
    let mut rpc = Rpc::connect(&input.rpc_url)?;
    let observed_genesis = rpc
        .call("getGenesisHash", &serde_json::json!([]))?
        .as_str()
        .ok_or_else(|| refusal("Series getGenesisHash returned a non-string"))?
        .to_owned();
    if observed_genesis != input.genesis_hash {
        return Err(refusal(
            "Series campaign changed its existing validator genesis",
        ));
    }
    if let Some(active) = journals.last().filter(|journal| {
        matches!(
            journal.phase,
            SeriesTerminalJournalPhaseV1::Dispatching | SeriesTerminalJournalPhaseV1::Submitted
        )
    }) {
        let journal_path = series_action_journal_path_v1(&arguments.journal_dir, active.sequence);
        if let Some((journal, conservation)) = try_finalize_landed_series_action_v1(
            &mut rpc,
            &journal_path,
            active,
            &source,
            lookup_table,
            &input.lookup_table_sha256,
        )? {
            if let Some(receipt) = conservation {
                create_series_canonical_json_v1(
                    &series_conservation_path_v1(&arguments.journal_dir, active.sequence),
                    &receipt,
                    "Series terminal conservation",
                )?;
            }
            print_series_terminal_progress_v1(
                &journal_path,
                &journal,
                "finalized",
                "The landed durable packet was authenticated against the current release and its same-ledger poststate; rerun to select the next act.",
            );
            return Ok(());
        }
    }
    if usize::try_from(sequence).ok() == Some(input.acquisition_recipes.len())
        && journals.last().is_some_and(|journal| {
            journal.phase == SeriesTerminalJournalPhaseV1::Finalized
                && journal.action == SeriesJournalActionV1::Close
        })
    {
        return run_series_market_retirement_phase_v1(
            &mut rpc,
            &input,
            &found,
            &campaign_sha256,
            &ledger,
            &journals,
            &arguments,
            payer,
            lookup_table,
        );
    }
    let frame = input
        .acquisition_recipes
        .get(usize::try_from(sequence).map_err(|_| refusal("Series sequence escaped usize"))?)
        .ok_or_else(|| refusal("Series acquisition manifest omitted the next planner sequence"))?;
    if frame.sequence != sequence {
        return Err(refusal(
            "Series acquisition recipe did not match the durable sequence",
        ));
    }
    let acquired =
        acquire_current_series_selected_v1(&mut rpc, frame, &source, payer, lookup_table)?;
    let _durable_frame = load_or_create_series_acquired_address_frame_v2(
        &arguments.journal_dir,
        sequence,
        frame,
        &campaign_sha256,
        &ledger,
        &acquired.selected,
    )?;
    let lookup_observed = operator_account_v1(
        required_series_account_v1(&acquired.accounts, lookup_table, "Series lookup table")?,
        acquired.observation,
    )?;
    authenticate_series_lookup_table_v1(
        &lookup_observed,
        lookup_table,
        &input.lookup_table_sha256,
    )?;
    authenticate_permissionless_series_signers_v1(&acquired.selected.instruction, payer)?;
    let journal_path = series_action_journal_path_v1(&arguments.journal_dir, sequence);
    let current = journals.last().filter(|journal| {
        journal.sequence == sequence && journal.phase != SeriesTerminalJournalPhaseV1::Finalized
    });
    let prepared = if let Some(current) = current {
        reauthenticate_series_selected_action_v1(current, &acquired.selected)?;
        current.clone()
    } else {
        let snapshot_sha256 =
            acquired_series_snapshot_digest_v1(acquired.observation, &acquired.accounts);
        let planned = plan_series_terminal_journal_v1(
            SeriesPlannerObservationV1 {
                campaign_sha256: campaign_sha256.clone(),
                ledger: ledger.clone(),
                finalized_slot: acquired.observation.slot,
                snapshot_sha256,
            },
            sequence,
            &acquired.lifecycle,
        )?;
        let prestate = selected_projection_from_acquisition_v1(
            &ledger,
            &acquired.selected,
            payer,
            acquired.observation.slot,
            &acquired.accounts,
        )?;
        let prepared =
            prepare_series_terminal_journal_v1(&planned, &acquired.selected, prestate, payer)?;
        authenticate_found_and_selected_v1(
            &found,
            &campaign_sha256,
            &ledger,
            &prepared,
            &acquired.selected,
        )?;
        create_series_terminal_journal_file_v1(&journal_path, &prepared)?;
        prepared
    };
    if !arguments.execute {
        print_series_terminal_progress_v1(
            &journal_path,
            &prepared,
            "prepared",
            "The exact current-source generic-Hot frame and same-slot prestate are durable; no key was read.",
        );
        return Ok(());
    }
    let payer_keypair = Keypair::new_from_array(read_keypair_file(
        &arguments.fee_payer_keypair,
        "Series fee payer",
    )?);
    if payer_keypair.pubkey() != payer {
        return Err(refusal(
            "Series fee-payer keypair did not name the campaign payer",
        ));
    }
    let active = match prepared.phase {
        SeriesTerminalJournalPhaseV1::Prepared => dispatch_series_terminal_from_rpc_v1(
            &mut rpc,
            &journal_path,
            &prepared,
            &acquired.selected,
            &payer_keypair,
            &[],
            lookup_table,
            &input.lookup_table_sha256,
            &lookup_observed,
        )?,
        SeriesTerminalJournalPhaseV1::Dispatching
        | SeriesTerminalJournalPhaseV1::Submitted
        | SeriesTerminalJournalPhaseV1::Finalized => prepared,
        SeriesTerminalJournalPhaseV1::Planned => {
            return Err(refusal(
                "Series command encountered a durable Planned journal",
            ));
        }
    };
    let advanced = advance_series_terminal_from_rpc_v1(
        &mut rpc,
        &journal_path,
        &active,
        &acquired.selected,
        lookup_table,
        &input.lookup_table_sha256,
        &lookup_observed,
    )?;
    match advanced {
        SeriesTerminalRpcAdvanceV1::Dispatching(journal)
        | SeriesTerminalRpcAdvanceV1::Pending(journal) => print_series_terminal_progress_v1(
            &journal_path,
            &journal,
            "pending",
            "The fsynced signature is the only transaction identity; rerun this command to poll it.",
        ),
        SeriesTerminalRpcAdvanceV1::Finalized {
            journal,
            conservation,
        } => {
            if let Some(receipt) = conservation {
                let receipt_path = series_conservation_path_v1(&arguments.journal_dir, sequence);
                create_series_canonical_json_v1(
                    &receipt_path,
                    &receipt,
                    "Series terminal conservation",
                )?;
            }
            print_series_terminal_progress_v1(
                &journal_path,
                &journal,
                "finalized",
                "The exact landed packet and same-ledger poststate are durable; rerun to select the next act.",
            );
        }
    }
    Ok(())
}

fn parse_series_terminal_arguments_v1(
    arguments: Vec<String>,
) -> Result<SeriesTerminalCampaignArgumentsV1> {
    let mut input = None;
    let mut journal_dir = None;
    let mut completion = None;
    let mut fee_payer_keypair = None;
    let mut execute = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--input" | "--journal-dir" | "--completion" | "--fee-payer-keypair" => {
                let flag = arguments[index].as_str();
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| refusal(format!("{flag} requires a value")))?;
                let slot = match flag {
                    "--input" => &mut input,
                    "--journal-dir" => &mut journal_dir,
                    "--completion" => &mut completion,
                    "--fee-payer-keypair" => &mut fee_payer_keypair,
                    _ => unreachable!(),
                };
                if slot.replace(PathBuf::from(value)).is_some() {
                    return Err(refusal(format!("{flag} may be supplied only once")));
                }
                index += 2;
            }
            "--execute" => {
                if execute {
                    return Err(refusal("--execute may be supplied only once"));
                }
                execute = true;
                index += 1;
            }
            unknown => {
                return Err(refusal(format!(
                    "unknown Series terminal campaign argument {unknown}"
                )));
            }
        }
    }
    let value = SeriesTerminalCampaignArgumentsV1 {
        input: input.ok_or_else(|| refusal("Series terminal campaign requires --input"))?,
        journal_dir: journal_dir
            .ok_or_else(|| refusal("Series terminal campaign requires --journal-dir"))?,
        completion: completion
            .ok_or_else(|| refusal("Series terminal campaign requires --completion"))?,
        fee_payer_keypair: fee_payer_keypair
            .ok_or_else(|| refusal("Series terminal campaign requires --fee-payer-keypair"))?,
        execute,
    };
    for path in [
        &value.input,
        &value.journal_dir,
        &value.completion,
        &value.fee_payer_keypair,
    ] {
        if !path.is_absolute() {
            return Err(refusal(
                "Series terminal campaign requires absolute durable paths",
            ));
        }
    }
    Ok(value)
}

fn authenticate_permissionless_series_signers_v1(
    instruction: &Instruction,
    payer: Pubkey,
) -> Result<()> {
    if instruction
        .accounts
        .iter()
        .any(|meta| meta.is_signer && meta.pubkey != payer)
    {
        return Err(refusal(
            "Series selected frame requires an additional signer not supplied by this permissionless campaign",
        ));
    }
    Ok(())
}

fn read_series_terminal_campaign_input_v1(
    path: &Path,
) -> Result<(SeriesTerminalCampaignInputV2, String)> {
    let bytes = read_bounded_series_file_v1(path, "Series terminal campaign input")?;
    let digest = sha256_hex(&bytes);
    let value = parse_json_without_duplicate_keys_v1(&bytes)?;
    let input = serde_json::from_value(value)
        .map_err(|error| Error::new(format!("Series terminal campaign input JSON: {error}")))?;
    Ok((input, digest))
}

fn authenticate_series_terminal_campaign_input_v1(
    input: &SeriesTerminalCampaignInputV2,
    arguments: &SeriesTerminalCampaignArgumentsV1,
    campaign_sha256: &str,
) -> Result<()> {
    require_sha256(campaign_sha256, "Series campaign input")?;
    require_sha256(&input.lookup_table_sha256, "Series lookup table")?;
    require_sha256(&input.prefix_ledger_sha256, "Series prefix ledger file")?;
    let prefix_path = Path::new(&input.prefix_ledger);
    if input.schema != SERIES_TERMINAL_CAMPAIGN_INPUT_SCHEMA_V2
        || !input.rpc_url.starts_with("http://127.0.0.1:")
        || input.genesis_hash.is_empty()
        || !prefix_path.is_absolute()
        || !prefix_path.is_file()
        || input.acquisition_recipes.len() < 7
        || input.market_retirements.len() < 2
        || !arguments.journal_dir.is_dir()
        || arguments.input == arguments.completion
        || arguments.completion.starts_with(&arguments.journal_dir)
    {
        return Err(refusal(
            "Series campaign input, prefix authority, loopback origin, durable paths, or complete recipe count changed",
        ));
    }
    let parent = arguments
        .completion
        .parent()
        .ok_or_else(|| refusal("Series completion omitted its parent directory"))?;
    if !parent.is_dir() {
        return Err(refusal(
            "Series completion parent was not an existing directory",
        ));
    }
    let payer = parse_pubkey(&input.fee_payer, "Series fee payer")?;
    let lookup_table = parse_pubkey(&input.lookup_table, "Series lookup table")?;
    if payer == lookup_table {
        return Err(refusal("Series fee payer aliased the frozen lookup table"));
    }
    for (index, frame) in input.acquisition_recipes.iter().enumerate() {
        if frame.sequence != u32::try_from(index).map_err(|_| refusal("too many Series recipes"))?
            || frame.fixed.addresses().iter().any(|value| value.is_empty())
            || frame.runtime_logical_accounts.is_empty()
            || frame.runtime_logical_accounts.iter().any(String::is_empty)
            || frame.consume_shadow.as_ref().is_some_and(|shadow| {
                shadow.addresses().iter().any(|value| value.is_empty())
                    || shadow.checked_manifest_sha256.is_empty()
                    || shadow.request_base64.is_empty()
            })
        {
            return Err(refusal(
                "Series acquisition recipes were not gap-free or address-complete",
            ));
        }
    }
    let mut all_retirement_paths = Vec::<&Path>::new();
    for (index, route) in input.market_retirements.iter().enumerate() {
        let expected_roles = SERIES_RETIREMENT_ROLES_V1
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        if route.ordinal != u32::try_from(index).map_err(|_| refusal("too many retirements"))?
            || route
                .role_addresses
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                != expected_roles
            || !route.snapshot.is_absolute()
            || !route.campaign.is_absolute()
            || !route.journal_dir.is_absolute()
            || !route.completion.is_absolute()
            || !route.journal_dir.is_dir()
        {
            return Err(refusal(
                "Series Market retirement routes changed order, exact roles, or durable paths",
            ));
        }
        for path in [&route.snapshot, &route.campaign, &route.completion] {
            let parent = path
                .parent()
                .ok_or_else(|| refusal("Series retirement output omitted its parent"))?;
            if !parent.is_dir() {
                return Err(refusal(
                    "Series retirement output parent was not an existing directory",
                ));
            }
        }
        let durable_paths = [
            route.snapshot.as_path(),
            route.campaign.as_path(),
            route.journal_dir.as_path(),
            route.completion.as_path(),
        ];
        for candidate in durable_paths {
            for existing in &all_retirement_paths {
                if candidate == *existing
                    || candidate.starts_with(existing)
                    || existing.starts_with(candidate)
                {
                    return Err(refusal("Series retirement durable paths aliased or nested"));
                }
            }
            for reserved in [
                arguments.input.as_path(),
                prefix_path,
                arguments.journal_dir.as_path(),
                arguments.completion.as_path(),
                arguments.fee_payer_keypair.as_path(),
            ] {
                if candidate == reserved
                    || candidate.starts_with(reserved)
                    || reserved.starts_with(candidate)
                {
                    return Err(refusal(
                        "Series retirement durable path overlapped campaign input, key, journal, or completion",
                    ));
                }
            }
            all_retirement_paths.push(candidate);
        }
        let mut addresses = BTreeSet::new();
        for (role, address) in &route.role_addresses {
            let key = parse_pubkey(address, &format!("Series retirement role {role}"))?;
            if !addresses.insert(key) {
                return Err(refusal(
                    "Series Market retirement acquisition roles aliased",
                ));
            }
        }
        if !addresses.insert(payer) || !addresses.insert(lookup_table) {
            return Err(refusal(
                "Series retirement acquisition roles aliased payer or lookup table",
            ));
        }
    }
    Ok(())
}

fn authenticate_found_and_selected_v1(
    input: &SeriesFoundInputV1,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
    prepared: &SeriesTerminalJournalV1,
    selected: &SeriesSelectedHotReportV5,
) -> Result<()> {
    let found = admit_series_found_binding_v1(
        campaign_sha256.to_owned(),
        ledger,
        parse_pubkey(&input.root, "Series Found root")?,
        parse_pubkey(&input.parent_market, "Series Found parent Market")?,
        input.parent_generation,
        parse_hex32_v1(&input.template, "Series Found Template")?,
        input.signature.clone(),
        input.finalized_slot,
        input.packet_sha256.clone(),
        input.poststate_sha256.clone(),
    )?;
    let physical = prepared
        .physical
        .as_ref()
        .ok_or_else(|| refusal("Prepared Series action omitted its physical report"))?;
    if found.root != selected.roles.root.to_string()
        || found.parent_market != selected.parent_market.to_string()
        || found.parent_market_generation != selected.parent_generation
        || found.template != prepared.template
        || physical.authority.release_set != hex32(selected.release_set)
    {
        return Err(refusal(
            "Series selected report changed its Found root, parent Market, generation, Template, or release",
        ));
    }
    Ok(())
}

/// Poll a durable signature before asking the now-advanced lifecycle to
/// reproduce its historical selected report. If it landed, current-source V5
/// reauthentication proves the exact action artifacts/request while the
/// fsynced packet and initially authenticated physical frame prove its keys.
fn try_finalize_landed_series_action_v1(
    rpc: &mut Rpc,
    path: &Path,
    current: &SeriesTerminalJournalV1,
    source: &DecodedSeriesCurrentSourceV1,
    lookup_table_key: Pubkey,
    lookup_table_sha256: &str,
) -> Result<
    Option<(
        SeriesTerminalJournalV1,
        Option<SeriesTerminalConservationReceiptV1>,
    )>,
> {
    authenticate_series_terminal_journal_v1(current)?;
    let packet = current
        .packet
        .as_ref()
        .ok_or_else(|| refusal("active Series journal omitted its durable packet"))?;
    let signature = Signature::from_str(&packet.signed.signature)
        .map_err(|_| refusal("active Series durable signature was malformed"))?;
    let Some(finalized) =
        rpc.finalized_signed_packet(series_rpc_label_v1(current.action), signature, false)?
    else {
        return Ok(None);
    };
    reauthenticate_current_series_release_v1(current, source)?;
    let (table_observation, mut tables) =
        rpc.finalized_observed_accounts(&[lookup_table_key], finalized.evidence.slot)?;
    let table = tables
        .pop()
        .ok_or_else(|| refusal("Series recovery omitted its frozen lookup table"))?;
    authenticate_series_lookup_table_v1(&table, lookup_table_key, lookup_table_sha256)?;
    let instruction = current
        .physical
        .as_ref()
        .ok_or_else(|| refusal("active Series journal omitted its physical frame"))?
        .instruction()?;
    Rpc::authenticate_signed_v0_packet(
        series_rpc_label_v1(current.action),
        std::slice::from_ref(&instruction),
        parse_pubkey(&packet.payer, "Series packet payer")?,
        &table,
        &packet.signed,
    )?;
    let resolved = resolve_series_packet_keys_v1(&packet.signed, lookup_table_key, &table)?;
    if resolved.iter().map(ToString::to_string).collect::<Vec<_>>() != packet.resolved_account_keys
        || packet.lookup_table != lookup_table_key.to_string()
        || packet.lookup_table_sha256 != lookup_table_sha256
        || table_observation.finality != Finality::Finalized
    {
        return Err(refusal(
            "Series landed recovery changed its signed routing projection",
        ));
    }
    if sha256_hex(&finalized.packet) != packet.signed.packet_sha256 {
        return Err(refusal(
            "Series landed transaction bytes differed from the durable packet",
        ));
    }
    let fee = finalized
        .evidence
        .fee_lamports
        .ok_or_else(|| refusal("finalized Series transaction omitted exact fee"))?;
    let compute = finalized
        .evidence
        .compute_units_consumed
        .ok_or_else(|| refusal("finalized Series transaction omitted compute units"))?;
    let submitted = if current.phase == SeriesTerminalJournalPhaseV1::Dispatching {
        let submitted = submit_series_terminal_journal_v1(current, &packet.signed.signature)?;
        replace_series_terminal_journal_file_v1(path, current, &submitted)?;
        submitted
    } else {
        current.clone()
    };
    let poststate =
        observe_durable_series_projection_from_rpc_v1(rpc, &submitted, finalized.evidence.slot)?;
    let (finalized_journal, conservation) = finalize_series_terminal_journal_v1(
        &submitted,
        finalized.evidence.signature,
        packet.signed.packet_sha256.clone(),
        fee,
        compute,
        poststate,
    )?;
    replace_series_terminal_journal_file_v1(path, &submitted, &finalized_journal)?;
    Ok(Some((finalized_journal, conservation)))
}

fn reauthenticate_current_series_release_v1(
    journal: &SeriesTerminalJournalV1,
    source: &DecodedSeriesCurrentSourceV1,
) -> Result<()> {
    let physical = journal
        .physical
        .as_ref()
        .ok_or_else(|| refusal("Series recovery omitted its physical authority"))?;
    let template = ContentId::new(parse_hex32_v1(&journal.template, "Series Template")?)
        .map_err(|_| refusal("Series recovery Template was zero"))?;
    let owned = emit_current_series_release_source_v5(source.input(template))
        .map_err(|_| refusal("Series current-source release emission refused during recovery"))?;
    let release = compile_series_release_v5(owned.as_source()).map_err(|_| {
        refusal("Series current-source release compilation refused during recovery")
    })?;
    let request = decode_base64(&journal.request_base64, "Series recovery request")?;
    let selected = authenticate_series_selected_action_v5(&release, owned.as_source(), &request)
        .map_err(|_| refusal("Series current-source selected authentication refused recovery"))?;
    if selected.action != kernel_action(journal.action)
        || selected.request_bytes != request
        || selected.roles.ticket.is_some() != physical.ticket.is_some()
        || selected.roles.rent_credit.is_some() != physical.rent_credit.is_some()
    {
        return Err(refusal(
            "Series current release changed the durable action, request, or role shape",
        ));
    }
    match selected.authority {
        SeriesOccurrenceAuthorityV5::Prepare {
            market,
            generation,
            release_set,
            parent_root,
        }
        | SeriesOccurrenceAuthorityV5::Consume {
            market,
            generation,
            release_set,
            parent_root,
        } => {
            if physical.occurrence_permit.is_some()
                || physical.occurrence_market != Some(Pubkey::new_from_array(market).to_string())
                || physical.occurrence_market_generation != Some(generation)
                || physical.root != Pubkey::new_from_array(parent_root).to_string()
                || physical.authority.release_set != hex32(release_set)
            {
                return Err(refusal(
                    "Series current occurrence authority changed during recovery",
                ));
            }
        }
        SeriesOccurrenceAuthorityV5::Expire {
            market,
            generation,
            release_set,
            parent_root,
            ..
        } => {
            if physical.occurrence_permit.is_none()
                || physical.occurrence_market != Some(Pubkey::new_from_array(market).to_string())
                || physical.occurrence_market_generation != Some(generation)
                || physical.root != Pubkey::new_from_array(parent_root).to_string()
                || physical.authority.release_set != hex32(release_set)
            {
                return Err(refusal(
                    "Series current Expire authority changed during recovery",
                ));
            }
        }
        SeriesOccurrenceAuthorityV5::Terminal => {
            if physical.occurrence_market.is_some()
                || physical.occurrence_market_generation.is_some()
                || physical.occurrence_permit.is_some()
            {
                return Err(refusal(
                    "Series terminal recovery acquired occurrence authority",
                ));
            }
        }
    }
    let authority = durable_authority_v1(SeriesSelectedAuthorityIdsV1 {
        release_set: parse_hex32_v1(
            &physical.authority.release_set,
            "Series durable release set",
        )?,
        program_set_v2: release.program_set_id,
        descriptor: hash(&selected.descriptor).to_bytes(),
        profile_v3: selected.artifact_ids.account_profile,
        request_profile: selected.artifact_ids.request_profile,
        lifecycle_policy: selected.artifact_ids.lifecycle,
        strategy: selected.artifact_ids.strategy,
        transition: selected.artifact_ids.transition,
        effect_v5: selected.artifact_ids.effect,
    })?;
    if authority != physical.authority {
        return Err(refusal(
            "Series current release no longer reproduced durable selected authority",
        ));
    }
    Ok(())
}

fn observe_durable_series_projection_from_rpc_v1(
    rpc: &mut Rpc,
    journal: &SeriesTerminalJournalV1,
    minimum_slot: u64,
) -> Result<SeriesChainProjectionV1> {
    let physical = journal
        .physical
        .as_ref()
        .ok_or_else(|| refusal("Series durable projection omitted physical frame"))?;
    let instruction = physical.instruction()?;
    let mut keys = instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect::<BTreeSet<_>>();
    keys.insert(parse_pubkey(
        &physical.parent_market,
        "Series parent Market",
    )?);
    if let Some(market) = &physical.occurrence_market {
        keys.insert(parse_pubkey(market, "Series occurrence Market")?);
    }
    if let Some(permit) = &physical.occurrence_permit {
        keys.insert(parse_pubkey(permit, "Series occurrence permit")?);
    }
    let payer = parse_pubkey(
        &journal
            .packet
            .as_ref()
            .ok_or_else(|| refusal("Series durable projection omitted packet"))?
            .payer,
        "Series packet payer",
    )?;
    if !keys.insert(payer) {
        return Err(refusal(
            "Series durable payer aliased projected protocol state",
        ));
    }
    let keys = keys.into_iter().collect::<Vec<_>>();
    let (slot, accounts) = rpc.finalized_accounts(&keys, minimum_slot)?;
    let observed = keys
        .into_iter()
        .zip(accounts)
        .map(|(key, account)| SeriesObservedAccountSlotV1 {
            key,
            account: account.map(|account| SeriesObservedAccountV1 {
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            }),
        })
        .collect();
    build_series_chain_projection_v1(&journal.ledger, slot, observed)
}

#[allow(clippy::too_many_arguments)]
fn run_series_market_retirement_phase_v1(
    rpc: &mut Rpc,
    input: &SeriesTerminalCampaignInputV2,
    found_input: &SeriesFoundInputV1,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
    journals: &[SeriesTerminalJournalV1],
    arguments: &SeriesTerminalCampaignArgumentsV1,
    payer: Pubkey,
    lookup_table: Pubkey,
) -> Result<()> {
    let expected = expected_series_market_retirements_v1(found_input, journals)?;
    if expected.len() != input.market_retirements.len() {
        return Err(refusal(
            "Series retirement acquisition count differed from consumed children plus parent",
        ));
    }
    let mut bindings = Vec::with_capacity(expected.len());
    for (index, (route, expected)) in input.market_retirements.iter().zip(&expected).enumerate() {
        let ordinal = u32::try_from(index).map_err(|_| refusal("too many Series retirements"))?;
        let binding_path =
            series_market_retirement_binding_path_v1(&arguments.journal_dir, ordinal);
        if binding_path.exists() {
            let binding: SeriesMarketRetirementBindingV1 =
                read_series_canonical_json_v1(&binding_path, "Series Market retirement binding")?;
            authenticate_series_market_retirement_binding_v1(&binding)?;
            if binding.market != expected.market.to_string()
                || binding.generation != expected.generation
                || binding.rent_credit != expected.rent_credit.to_string()
                || binding.selected_release_set != hex32(expected.release_set)
            {
                return Err(refusal(
                    "Series durable Market retirement binding changed expected order or authority",
                ));
            }
            bindings.push(binding);
            continue;
        }
        let durable_snapshot = load_or_create_series_retirement_snapshot_v1(
            rpc,
            route,
            campaign_sha256,
            ledger,
            payer,
            lookup_table,
        )?;
        let snapshot = market_retirement_snapshot_from_durable_v1(&durable_snapshot)?;
        if snapshot.market.key != expected.market
            || snapshot.rent_credit.key != expected.rent_credit
        {
            return Err(refusal(
                "Series Market retirement snapshot changed its expected Market or RentCredit",
            ));
        }
        let report = build_checkpoint_market_retirement_v1(&snapshot)
            .map_err(|error| refusal(format!("Series generic Market retirement: {error:?}")))?;
        let campaign = load_or_create_series_aggregate_campaign_v1(
            route,
            input,
            campaign_sha256,
            payer,
            lookup_table,
            &durable_snapshot,
            &snapshot,
            &report,
        )?;
        run_authenticated_aggregate_retirement_v1(
            rpc,
            &campaign,
            AggregateRetirementTransportV1 {
                campaign_path: &route.campaign,
                journal_dir: &route.journal_dir,
                completion: &route.completion,
                payer,
                payer_keypair: &arguments.fee_payer_keypair,
                lookup_table,
                execute: arguments.execute,
            },
        )?;
        if !route.completion.exists() {
            return Ok(());
        }
        let receipt: AggregateRetirementConservationReceiptV1 =
            read_series_canonical_json_v1(&route.completion, "aggregate retirement completion")?;
        authenticate_aggregate_retirement_conservation_receipt_v1(&receipt)?;
        let binding = bind_series_market_retirement_v1(
            ledger,
            expected.release_set,
            expected.market,
            expected.generation,
            expected.rent_credit,
            &snapshot,
            &campaign,
            &receipt,
        )?;
        create_series_canonical_json_v1(
            &binding_path,
            &binding,
            "Series Market retirement binding",
        )?;
        println!(
            "{}",
            serde_json::json!({
                "schema": "dclutch-owned-loopback-series-terminal-progress-v1",
                "status": "market-retirement-finalized",
                "ordinal": ordinal,
                "market": binding.market,
                "generation": binding.generation,
                "binding": binding_path,
                "bindingSha256": binding.binding_sha256,
                "next": "Rerun the same command to authenticate the next consumed child Market, or the parent Market last.",
            })
        );
        return Ok(());
    }
    let conservation = load_series_conservation_receipts_v1(
        &arguments.journal_dir,
        journals,
        campaign_sha256,
        ledger,
    )?;
    let found = admit_series_found_binding_v1(
        campaign_sha256.to_owned(),
        ledger,
        parse_pubkey(&found_input.root, "Series Found root")?,
        parse_pubkey(&found_input.parent_market, "Series Found parent Market")?,
        found_input.parent_generation,
        parse_hex32_v1(&found_input.template, "Series Found Template")?,
        found_input.signature.clone(),
        found_input.finalized_slot,
        found_input.packet_sha256.clone(),
        found_input.poststate_sha256.clone(),
    )?;
    let completion = build_series_complete_lifecycle_ledger_v1(
        found,
        ledger.clone(),
        journals,
        &conservation,
        &bindings,
    )?;
    if arguments.completion.exists() {
        let durable: SeriesCompleteLifecycleLedgerV1 = read_series_canonical_json_v1(
            &arguments.completion,
            "Series complete lifecycle ledger",
        )?;
        authenticate_series_complete_lifecycle_ledger_v1(&durable)?;
        if durable != completion {
            return Err(refusal(
                "Series durable completion differed from fresh authenticated convergence",
            ));
        }
    } else {
        create_series_canonical_json_v1(
            &arguments.completion,
            &completion,
            "Series complete lifecycle ledger",
        )?;
    }
    println!(
        "{}",
        serde_json::json!({
            "schema": "dclutch-owned-loopback-series-terminal-progress-v1",
            "status": "complete",
            "completion": arguments.completion,
            "ledgerSha256": completion.ledger_sha256,
            "actions": completion.actions.len(),
            "marketRetirements": completion.market_retirements.len(),
            "temporaryProtocolStateClosed": completion.temporary_protocol_state_closed,
        })
    );
    Ok(())
}

struct ExpectedSeriesMarketRetirementV1 {
    market: Pubkey,
    generation: u64,
    release_set: [u8; 32],
    rent_credit: Pubkey,
}

fn expected_series_market_retirements_v1(
    found: &SeriesFoundInputV1,
    journals: &[SeriesTerminalJournalV1],
) -> Result<Vec<ExpectedSeriesMarketRetirementV1>> {
    let close = journals
        .last()
        .filter(|journal| {
            journal.phase == SeriesTerminalJournalPhaseV1::Finalized
                && journal.action == SeriesJournalActionV1::Close
        })
        .ok_or_else(|| refusal("Series Market retirement preceded finalized root Close"))?;
    let close_physical = close
        .physical
        .as_ref()
        .ok_or_else(|| refusal("Series Close omitted physical authority"))?;
    let release_set = parse_hex32_v1(
        &close_physical.authority.release_set,
        "Series selected release set",
    )?;
    let rent_credit = parse_pubkey(
        close_physical
            .rent_credit
            .as_ref()
            .ok_or_else(|| refusal("Series Close omitted RentCredit"))?,
        "Series lifecycle RentCredit",
    )?;
    let mut expected = Vec::new();
    for journal in journals {
        let physical = journal
            .physical
            .as_ref()
            .ok_or_else(|| refusal("Series finalized action omitted physical authority"))?;
        if physical.authority.release_set != close_physical.authority.release_set {
            return Err(refusal(
                "Series finalized actions crossed selected release sets",
            ));
        }
        if journal.action == SeriesJournalActionV1::Consume {
            expected.push(ExpectedSeriesMarketRetirementV1 {
                market: parse_pubkey(
                    physical
                        .occurrence_market
                        .as_ref()
                        .ok_or_else(|| refusal("Series Consume omitted child Market"))?,
                    "Series child Market",
                )?,
                generation: physical
                    .occurrence_market_generation
                    .ok_or_else(|| refusal("Series Consume omitted child generation"))?,
                release_set,
                rent_credit,
            });
        }
    }
    expected.push(ExpectedSeriesMarketRetirementV1 {
        market: parse_pubkey(&found.parent_market, "Series parent Market")?,
        generation: found.parent_generation,
        release_set,
        rent_credit,
    });
    Ok(expected)
}

fn load_or_create_series_retirement_snapshot_v1(
    rpc: &mut Rpc,
    route: &SeriesMarketRetirementRouteV1,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
    payer: Pubkey,
    lookup_table: Pubkey,
) -> Result<DurableSeriesMarketRetirementSnapshotV1> {
    if route.snapshot.exists() {
        let snapshot: DurableSeriesMarketRetirementSnapshotV1 =
            read_series_canonical_json_v1(&route.snapshot, "Series retirement snapshot")?;
        authenticate_durable_series_retirement_snapshot_v1(
            &snapshot,
            route,
            campaign_sha256,
            ledger,
            payer,
            lookup_table,
        )?;
        return Ok(snapshot);
    }
    let mut keys = route
        .role_addresses
        .values()
        .map(|value| parse_pubkey(value, "Series retirement acquisition role"))
        .collect::<Result<BTreeSet<_>>>()?;
    keys.insert(payer);
    keys.insert(lookup_table);
    let keys = keys.into_iter().collect::<Vec<_>>();
    let (slot, values) = rpc.finalized_accounts(&keys, 0)?;
    if slot == 0 || values.len() != keys.len() {
        return Err(refusal(
            "Series retirement snapshot was not one complete finalized vector",
        ));
    }
    let mut accounts = BTreeMap::new();
    for (key, account) in keys.into_iter().zip(values) {
        let durable = match account {
            Some(account) => durable_present_account_v1(SeriesObservedAccountV1 {
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            })?,
            None => durable_absent_account_v1(key),
        };
        accounts.insert(key.to_string(), durable);
    }
    let mut snapshot = DurableSeriesMarketRetirementSnapshotV1 {
        schema: SERIES_MARKET_RETIREMENT_SNAPSHOT_SCHEMA_V1.into(),
        campaign_sha256: campaign_sha256.to_owned(),
        ledger_identity_sha256: ledger.identity_sha256.clone(),
        ordinal: route.ordinal,
        payer: payer.to_string(),
        lookup_table: lookup_table.to_string(),
        observation_slot: slot,
        observation_unix_timestamp: rpc.block_time(slot)?,
        role_addresses: route.role_addresses.clone(),
        accounts,
        snapshot_sha256: String::new(),
    };
    snapshot.snapshot_sha256 = series_retirement_snapshot_digest_v1(&snapshot)?;
    authenticate_durable_series_retirement_snapshot_v1(
        &snapshot,
        route,
        campaign_sha256,
        ledger,
        payer,
        lookup_table,
    )?;
    create_series_canonical_json_v1(&route.snapshot, &snapshot, "Series retirement snapshot")?;
    Ok(snapshot)
}

fn authenticate_durable_series_retirement_snapshot_v1(
    snapshot: &DurableSeriesMarketRetirementSnapshotV1,
    route: &SeriesMarketRetirementRouteV1,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
    payer: Pubkey,
    lookup_table: Pubkey,
) -> Result<()> {
    if snapshot.schema != SERIES_MARKET_RETIREMENT_SNAPSHOT_SCHEMA_V1
        || snapshot.campaign_sha256 != campaign_sha256
        || snapshot.ledger_identity_sha256 != ledger.identity_sha256
        || snapshot.ordinal != route.ordinal
        || snapshot.payer != payer.to_string()
        || snapshot.lookup_table != lookup_table.to_string()
        || snapshot.observation_slot == 0
        || snapshot.role_addresses != route.role_addresses
        || snapshot.snapshot_sha256 != series_retirement_snapshot_digest_v1(snapshot)?
    {
        return Err(refusal(
            "Series durable retirement snapshot changed identity, observation, or routing",
        ));
    }
    if snapshot.accounts.len() != snapshot.role_addresses.len().saturating_add(2) {
        return Err(refusal(
            "Series retirement snapshot changed its complete account cardinality",
        ));
    }
    for (address, account) in &snapshot.accounts {
        if address != &account.address {
            return Err(refusal(
                "Series retirement snapshot map key changed its account identity",
            ));
        }
        authenticate_durable_account_v1(account)?;
    }
    if !snapshot.accounts.contains_key(&snapshot.payer)
        || !snapshot.accounts.contains_key(&snapshot.lookup_table)
    {
        return Err(refusal(
            "Series retirement snapshot omitted payer or lookup table",
        ));
    }
    for address in snapshot.role_addresses.values() {
        if !snapshot.accounts.contains_key(address) {
            return Err(refusal("Series retirement snapshot omitted a routed role"));
        }
    }
    Ok(())
}

fn series_retirement_snapshot_digest_v1(
    snapshot: &DurableSeriesMarketRetirementSnapshotV1,
) -> Result<String> {
    let mut value = snapshot.clone();
    value.snapshot_sha256.clear();
    let bytes = serde_json::to_vec(&value)
        .map_err(|error| Error::new(format!("serialize Series retirement snapshot: {error}")))?;
    Ok(sha256_hex(&bytes))
}

fn market_retirement_snapshot_from_durable_v1(
    durable: &DurableSeriesMarketRetirementSnapshotV1,
) -> Result<MarketRetirementSnapshotV1> {
    let observation = dclutch_market_retirement_v1_operator::Observation {
        slot: durable.observation_slot,
        unix_timestamp: durable.observation_unix_timestamp,
        finality: dclutch_market_retirement_v1_operator::Finality::Finalized,
    };
    let account = |role: &str| -> Result<dclutch_market_retirement_v1_operator::ObservedAccount> {
        let address = durable
            .role_addresses
            .get(role)
            .ok_or_else(|| refusal(format!("Series retirement omitted role {role}")))?;
        durable_retirement_observed_account_v1(
            durable
                .accounts
                .get(address)
                .ok_or_else(|| refusal(format!("Series retirement omitted account {role}")))?,
            observation,
        )
    };
    Ok(MarketRetirementSnapshotV1 {
        market: account("market")?,
        rent_credit: account("rent-credit")?,
        activation_cache: account("activation-cache")?,
        registry_program: account("registry-program")?,
        core_program: account("core-program")?,
        core_programdata: account("core-programdata")?,
        claims_program: account("claims-program")?,
        claims_programdata: account("claims-programdata")?,
        resolution_program: account("resolution-program")?,
        resolution_programdata: account("resolution-programdata")?,
        custody_program: account("custody-program")?,
        custody_programdata: account("custody-programdata")?,
        rent_program: account("rent-program")?,
        source_receipt: account("source-receipt")?,
        claims_aggregate: account("claims-aggregate")?,
        custody_replay: account("custody-replay")?,
        hoard_vault: account("hoard-vault")?,
        custody_authority: account("custody-authority")?,
        collateral_mint: account("collateral-mint")?,
        collateral_token_program: account("collateral-token-program")?,
        realm_raw: account("realm-raw")?,
        realm_staging: account("realm-staging")?,
        infrastructure_profile: account("infrastructure-profile")?,
        registry_artifact_raw: account("registry-artifact-raw")?,
        registry_artifact_staging: account("registry-artifact-staging")?,
        registry_programdata: account("registry-programdata")?,
        rent_artifact_raw: account("rent-artifact-raw")?,
        rent_artifact_staging: account("rent-artifact-staging")?,
        rent_programdata: account("rent-programdata")?,
        rent_sysvar: account("rent-sysvar")?,
        refund_wallet: account("refund-wallet")?,
    })
}

fn durable_retirement_observed_account_v1(
    durable: &DurableSeriesAccountV1,
    observation: dclutch_market_retirement_v1_operator::Observation,
) -> Result<dclutch_market_retirement_v1_operator::ObservedAccount> {
    authenticate_durable_account_v1(durable)?;
    Ok(dclutch_market_retirement_v1_operator::ObservedAccount {
        observation,
        key: parse_pubkey(&durable.address, "Series retirement account")?,
        owner: match durable.owner.as_ref() {
            Some(owner) => parse_pubkey(owner, "Series retirement owner")?,
            None => solana_sdk_ids::system_program::ID,
        },
        lamports: durable.lamports.unwrap_or(0),
        executable: durable.executable.unwrap_or(false),
        data: match durable.data_base64.as_ref() {
            Some(data) => decode_base64(data, "Series retirement account")?,
            None => Vec::new(),
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn load_or_create_series_aggregate_campaign_v1(
    route: &SeriesMarketRetirementRouteV1,
    input: &SeriesTerminalCampaignInputV2,
    campaign_sha256: &str,
    payer: Pubkey,
    lookup_table: Pubkey,
    durable_snapshot: &DurableSeriesMarketRetirementSnapshotV1,
    snapshot: &MarketRetirementSnapshotV1,
    report: &dclutch_market_retirement_v1_operator::CheckpointMarketRetirementReportV1,
) -> Result<AggregateRetirementCampaignV1> {
    if route.campaign.exists() {
        let campaign: AggregateRetirementCampaignV1 =
            read_series_canonical_json_v1(&route.campaign, "aggregate retirement campaign")?;
        authenticate_aggregate_retirement_campaign_v1(&campaign)?;
        if campaign.plan_sha256 != campaign_sha256
            || campaign.evidence_sha256 != durable_snapshot.snapshot_sha256
        {
            return Err(refusal(
                "Series aggregate retirement campaign changed its source evidence",
            ));
        }
        return Ok(campaign);
    }
    let table = durable_snapshot
        .accounts
        .get(&lookup_table.to_string())
        .ok_or_else(|| refusal("Series retirement snapshot omitted lookup table"))?;
    let table_data = table
        .data_base64
        .as_ref()
        .ok_or_else(|| refusal("Series retirement lookup table was absent"))?;
    let observed_table_sha256 = sha256_hex(&decode_base64(
        table_data,
        "Series retirement lookup table",
    )?);
    if observed_table_sha256 != input.lookup_table_sha256 {
        return Err(refusal(
            "Series retirement lookup table changed from campaign admission",
        ));
    }
    let initial = |account: &dclutch_market_retirement_v1_operator::ObservedAccount| {
        AggregateRetirementInitialAccountV1 {
            key: account.key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data.clone(),
        }
    };
    let campaign = build_aggregate_retirement_campaign_v1(
        AggregateRetirementCampaignInputV1 {
            genesis_hash: input.genesis_hash.clone(),
            rpc_url: input.rpc_url.clone(),
            plan_sha256: campaign_sha256.to_owned(),
            evidence_sha256: durable_snapshot.snapshot_sha256.clone(),
            payer,
            lookup_table,
            lookup_table_sha256: observed_table_sha256,
            core_program: snapshot.core_program.key,
            claims_program: snapshot.claims_program.key,
            market: initial(&snapshot.market),
            rent_credit: initial(&snapshot.rent_credit),
            checkpoint: initial(&snapshot.claims_aggregate),
            custody_replay: initial(&snapshot.custody_replay),
            hoard_vault: initial(&snapshot.hoard_vault),
            source_receipt: initial(&snapshot.source_receipt),
            refund_wallet: initial(&snapshot.refund_wallet),
        },
        report,
    )?;
    create_series_canonical_json_v1(&route.campaign, &campaign, "aggregate retirement campaign")?;
    Ok(campaign)
}

fn load_series_conservation_receipts_v1(
    directory: &Path,
    journals: &[SeriesTerminalJournalV1],
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
) -> Result<Vec<SeriesTerminalConservationReceiptV1>> {
    journals
        .iter()
        .filter(|journal| journal.action.terminal())
        .map(|journal| {
            let receipt: SeriesTerminalConservationReceiptV1 = read_series_canonical_json_v1(
                &series_conservation_path_v1(directory, journal.sequence),
                "Series terminal conservation",
            )?;
            authenticate_conservation_receipt_v1(&receipt)?;
            if receipt.campaign_sha256 != campaign_sha256
                || receipt.ledger_identity_sha256 != ledger.identity_sha256
            {
                return Err(refusal(
                    "Series conservation receipt changed campaign or ledger",
                ));
            }
            Ok(receipt)
        })
        .collect()
}

fn series_market_retirement_binding_path_v1(directory: &Path, ordinal: u32) -> PathBuf {
    directory.join(format!("series-market-retirement-{ordinal:08}.json"))
}

fn load_series_terminal_journals_v1(
    directory: &Path,
    frame_count: usize,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
) -> Result<Vec<SeriesTerminalJournalV1>> {
    let mut journals = Vec::new();
    for index in 0..frame_count {
        let sequence = u32::try_from(index).map_err(|_| refusal("too many Series journals"))?;
        let path = series_action_journal_path_v1(directory, sequence);
        if !path.exists() {
            break;
        }
        let journal = read_series_terminal_journal_file_v1(&path)?;
        if journal.sequence != sequence
            || journal.campaign_sha256 != campaign_sha256
            || journal.ledger.identity_sha256 != ledger.identity_sha256
        {
            return Err(refusal(
                "Series durable journal changed campaign, ledger, or sequence",
            ));
        }
        if journals
            .last()
            .is_some_and(|previous: &SeriesTerminalJournalV1| {
                previous.phase != SeriesTerminalJournalPhaseV1::Finalized
            })
        {
            return Err(refusal("Series journal gap followed an active action"));
        }
        journals.push(journal);
    }
    let known = journals.len();
    if (known..frame_count).skip(1).any(|index| {
        series_action_journal_path_v1(directory, u32::try_from(index).unwrap_or(u32::MAX)).exists()
    }) {
        return Err(refusal("Series journal directory contained a sequence gap"));
    }
    Ok(journals)
}

fn series_action_journal_path_v1(directory: &Path, sequence: u32) -> PathBuf {
    directory.join(format!("series-action-{sequence:08}.json"))
}

fn series_conservation_path_v1(directory: &Path, sequence: u32) -> PathBuf {
    directory.join(format!("series-conservation-{sequence:08}.json"))
}

fn series_acquired_address_frame_path_v2(directory: &Path, sequence: u32) -> PathBuf {
    directory.join(format!("series-acquired-frame-{sequence:08}.json"))
}

fn authenticate_series_acquired_address_frame_history_v2(
    directory: &Path,
    recipes: &[SeriesHotAcquisitionRecipeV2],
    journals: &[SeriesTerminalJournalV1],
    current_sequence: u32,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
) -> Result<()> {
    for recipe in recipes {
        let path = series_acquired_address_frame_path_v2(directory, recipe.sequence);
        let Some(journal) = journals.get(
            usize::try_from(recipe.sequence)
                .map_err(|_| refusal("Series acquired-frame sequence escaped usize"))?,
        ) else {
            if recipe.sequence > current_sequence && path.exists() {
                return Err(refusal(
                    "Series acquisition preauthored a future durable bank",
                ));
            }
            if recipe.sequence < current_sequence {
                return Err(refusal(
                    "Series acquired-frame history skipped a durable journal",
                ));
            }
            if recipe.sequence == current_sequence && path.exists() {
                let frame: SeriesAcquiredAddressFrameV2 =
                    read_series_canonical_json_v1(&path, "Series current acquired address frame")?;
                authenticate_series_acquired_address_frame_shape_v2(
                    &frame,
                    recipe,
                    campaign_sha256,
                    ledger,
                )?;
            }
            continue;
        };
        if !path.is_file() {
            return Err(refusal(
                "Series finalized journal omitted its durable acquired address frame",
            ));
        }
        let frame: SeriesAcquiredAddressFrameV2 =
            read_series_canonical_json_v1(&path, "Series acquired address frame")?;
        authenticate_series_acquired_address_frame_shape_v2(
            &frame,
            recipe,
            campaign_sha256,
            ledger,
        )?;
        let physical = journal
            .physical
            .as_ref()
            .ok_or_else(|| refusal("Series journal omitted acquired physical authority"))?;
        if frame.action != journal.action
            || frame.request_sha256 != journal.request_sha256
            || frame.selected_release_set != physical.authority.release_set
            || frame.observation_slot != journal.planner_finalized_slot
        {
            return Err(refusal(
                "Series acquired address frame changed from its durable action journal",
            ));
        }
    }
    Ok(())
}

fn load_or_create_series_acquired_address_frame_v2<S: SelectedSeriesPhysicalActionV1>(
    directory: &Path,
    current_sequence: u32,
    recipe: &SeriesHotAcquisitionRecipeV2,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
    selected: &S,
) -> Result<SeriesAcquiredAddressFrameV2> {
    if recipe.sequence != current_sequence {
        return Err(refusal(
            "Series acquisition cannot persist a non-current sequence",
        ));
    }
    let path = series_acquired_address_frame_path_v2(directory, recipe.sequence);
    if !path.exists() {
        let mut frame = SeriesAcquiredAddressFrameV2 {
            schema: SERIES_ACQUIRED_ADDRESS_FRAME_SCHEMA_V2.to_owned(),
            campaign_sha256: campaign_sha256.to_owned(),
            ledger_identity_sha256: ledger.identity_sha256.clone(),
            sequence: recipe.sequence,
            observation_slot: selected.observation().slot,
            observation_unix_timestamp: selected.observation().unix_timestamp,
            action: SeriesJournalActionV1::from_kernel(selected.action()),
            request_sha256: sha256_hex(selected.canonical_request_bytes()),
            selected_release_set: hex32(selected.selected_authority_ids().release_set),
            recipe: recipe.clone(),
            frame_sha256: String::new(),
        };
        frame.frame_sha256 = acquired_address_frame_digest_v2(&frame)?;
        authenticate_series_acquired_address_frame_v2(
            &frame,
            recipe,
            campaign_sha256,
            ledger,
            selected,
        )?;
        create_series_canonical_json_v1(&path, &frame, "Series acquired address frame")?;
    }
    let frame: SeriesAcquiredAddressFrameV2 =
        read_series_canonical_json_v1(&path, "Series acquired address frame")?;
    authenticate_series_acquired_address_frame_v2(
        &frame,
        recipe,
        campaign_sha256,
        ledger,
        selected,
    )?;
    Ok(frame)
}

fn authenticate_series_acquired_address_frame_v2<S: SelectedSeriesPhysicalActionV1>(
    frame: &SeriesAcquiredAddressFrameV2,
    recipe: &SeriesHotAcquisitionRecipeV2,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
    selected: &S,
) -> Result<()> {
    authenticate_series_acquired_address_frame_shape_v2(frame, recipe, campaign_sha256, ledger)?;
    if frame.observation_slot != selected.observation().slot
        || frame.observation_unix_timestamp != selected.observation().unix_timestamp
        || frame.action != SeriesJournalActionV1::from_kernel(selected.action())
        || frame.request_sha256 != sha256_hex(selected.canonical_request_bytes())
        || frame.selected_release_set != hex32(selected.selected_authority_ids().release_set)
    {
        return Err(refusal(
            "Series acquired address frame changed its canonical current observation, route, request, or release",
        ));
    }
    Ok(())
}

fn authenticate_series_acquired_address_frame_shape_v2(
    frame: &SeriesAcquiredAddressFrameV2,
    recipe: &SeriesHotAcquisitionRecipeV2,
    campaign_sha256: &str,
    ledger: &SeriesLedgerIdentityV1,
) -> Result<()> {
    require_sha256(&frame.campaign_sha256, "Series acquired-frame campaign")?;
    require_sha256(
        &frame.ledger_identity_sha256,
        "Series acquired-frame ledger",
    )?;
    require_sha256(&frame.request_sha256, "Series acquired-frame request")?;
    require_sha256(
        &frame.selected_release_set,
        "Series acquired-frame selected release",
    )?;
    require_sha256(&frame.frame_sha256, "Series acquired-frame digest")?;
    if frame.schema != SERIES_ACQUIRED_ADDRESS_FRAME_SCHEMA_V2
        || frame.campaign_sha256 != campaign_sha256
        || frame.ledger_identity_sha256 != ledger.identity_sha256
        || frame.sequence != recipe.sequence
        || frame.recipe != *recipe
        || frame.observation_slot == 0
        || frame.frame_sha256 != acquired_address_frame_digest_v2(frame)?
    {
        return Err(refusal(
            "Series acquired address frame changed its campaign, ledger, sequence, address recipe, or digest",
        ));
    }
    Ok(())
}

fn acquired_address_frame_digest_v2(frame: &SeriesAcquiredAddressFrameV2) -> Result<String> {
    let mut projected = frame.clone();
    projected.frame_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&projected)?))
}

fn selected_projection_from_acquisition_v1(
    ledger: &SeriesLedgerIdentityV1,
    selected: &SeriesSelectedHotReportV5,
    payer: Pubkey,
    slot: u64,
    accounts: &BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
) -> Result<SeriesChainProjectionV1> {
    let mut keys = selected
        .instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect::<BTreeSet<_>>();
    keys.insert(selected.parent_market);
    if let Some(market) = selected.roles.occurrence_market {
        keys.insert(market);
    }
    if let Some(permit) = selected.roles.permit {
        keys.insert(permit);
    }
    if !keys.insert(payer) {
        return Err(refusal(
            "Series acquisition fee payer aliased projected protocol state",
        ));
    }
    let observed = keys
        .into_iter()
        .map(|key| {
            let account = accounts
                .get(&key)
                .ok_or_else(|| refusal("Series acquisition omitted projected account"))?
                .clone();
            Ok(SeriesObservedAccountSlotV1 { key, account })
        })
        .collect::<Result<Vec<_>>>()?;
    build_series_chain_projection_v1(ledger, slot, observed)
}

fn acquired_series_snapshot_digest_v1(
    observation: Observation,
    accounts: &BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"dclutch/series-terminal/planner-snapshot/v1\0");
    digest.update(observation.slot.to_le_bytes());
    digest.update(observation.unix_timestamp.to_le_bytes());
    digest.update([u8::from(observation.finality == Finality::Finalized)]);
    for (key, account) in accounts {
        digest.update(key.as_ref());
        match account {
            Some(account) => {
                digest.update([1]);
                digest.update(account.owner.as_ref());
                digest.update(account.lamports.to_le_bytes());
                digest.update([u8::from(account.executable)]);
                digest.update(
                    u64::try_from(account.data.len())
                        .unwrap_or(u64::MAX)
                        .to_le_bytes(),
                );
                digest.update(&account.data);
            }
            None => digest.update([0]),
        }
    }
    format!("{:x}", digest.finalize())
}

fn create_series_canonical_json_v1<T: Serialize>(
    path: &Path,
    value: &T,
    label: &str,
) -> Result<()> {
    require_absolute_series_output_v1(path)?;
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| Error::new(format!("serialize {label}: {error}")))?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| Error::new(format!("create {label} {}: {error}", path.display())))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    sync_series_parent_v1(path)
}

fn read_series_canonical_json_v1<T: DeserializeOwned + Serialize>(
    path: &Path,
    label: &str,
) -> Result<T> {
    let source = read_bounded_series_file_v1(path, label)?;
    let value = parse_json_without_duplicate_keys_v1(&source)?;
    let decoded = serde_json::from_value(value)
        .map_err(|error| Error::new(format!("{label} JSON: {error}")))?;
    let mut canonical = serde_json::to_vec(&decoded)
        .map_err(|error| Error::new(format!("serialize {label}: {error}")))?;
    canonical.push(b'\n');
    if source != canonical {
        return Err(refusal(format!("{label} was not canonical durable JSON")));
    }
    Ok(decoded)
}

fn print_series_terminal_progress_v1(
    path: &Path,
    journal: &SeriesTerminalJournalV1,
    status: &str,
    next: &str,
) {
    println!(
        "{}",
        serde_json::json!({
            "schema": "dclutch-owned-loopback-series-terminal-progress-v1",
            "status": status,
            "sequence": journal.sequence,
            "action": journal.action,
            "phase": journal.phase,
            "journal": path,
            "journalSha256": journal.state_sha256,
            "next": next,
        })
    );
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-series-terminal-campaign-v1 \\\n  --input ABSOLUTE_JSON --journal-dir ABSOLUTE_EXISTING_DIR \\\n  --completion ABSOLUTE_NEW_JSON --fee-payer-keypair ABSOLUTE_JSON [--execute]\n\
\nThe input binds an authenticated lifecycle-prefix-v2 Found and address-only acquisition recipes, \
never an action, privilege bank, or selected-release DTO. Each invocation observes only the current \
recipe, lets the Series lifecycle planner choose the act, re-emits the exact current five-entry V5 \
release, and authenticates Profile13 packing and runtime roles through SeriesSelectedHotReportV5. It \
then appends and rereads one address-only acquired frame; a future durable frame is refused. Prepared \
is the first durable action-journal boundary. Without --execute it never opens the key. With --execute \
it fsyncs one signed v0 packet before send and thereafter only polls or resends those exact bytes. \
Loopback RPC is mandatory."
}

/// Host account fact captured directly from one finalized RPC observation.
#[derive(Clone, Debug)]
pub(crate) struct SeriesObservedAccountV1 {
    pub(crate) key: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data: Vec<u8>,
}

/// A present account or exact absence at one finalized slot.
#[derive(Clone, Debug)]
pub(crate) struct SeriesObservedAccountSlotV1 {
    pub(crate) key: Pubkey,
    pub(crate) account: Option<SeriesObservedAccountV1>,
}

/// Content-complete durable account fact. Zero-lamport deletion is represented
/// as absence, not a fabricated system-owned empty account.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableSeriesAccountV1 {
    pub(crate) address: String,
    pub(crate) present: bool,
    pub(crate) owner: Option<String>,
    pub(crate) lamports: Option<u64>,
    pub(crate) executable: Option<bool>,
    pub(crate) data_base64: Option<String>,
    pub(crate) data_sha256: Option<String>,
    pub(crate) account_sha256: String,
}

/// Exact protocol-account projection used at a crash or finalization boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesChainProjectionV1 {
    pub(crate) ledger_identity_sha256: String,
    pub(crate) finalized_slot: u64,
    pub(crate) accounts: BTreeMap<String, DurableSeriesAccountV1>,
    pub(crate) state_sha256: String,
}

/// Ordered account metadata retained from the canonical selected instruction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableSeriesInstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
}

/// Selected-release authority retained beside physical bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableSeriesSelectedAuthorityV1 {
    release_set: String,
    program_set_v2: String,
    descriptor: String,
    profile_v3: String,
    request_profile: String,
    lifecycle_policy: String,
    strategy: String,
    transition: String,
    effect_v5: String,
    authority_sha256: String,
}

/// Exact physical generic-Hot action. The instruction is evidence copied from
/// the selected API; this module does not know or invent its account geometry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableSeriesPhysicalActionV1 {
    route: String,
    mechanism: SeriesPhysicalMechanismV1,
    trading_program: String,
    accounts: Vec<DurableSeriesInstructionAccountV1>,
    data_base64: String,
    data_sha256: String,
    request_sha256: String,
    authority: DurableSeriesSelectedAuthorityV1,
    root: String,
    ticket: Option<String>,
    rent_credit: Option<String>,
    parent_market: String,
    parent_market_generation: u64,
    occurrence_market: Option<String>,
    occurrence_market_generation: Option<u64>,
    occurrence_permit: Option<String>,
    physical_sha256: String,
}

impl DurableSeriesPhysicalActionV1 {
    pub(crate) fn instruction(&self) -> Result<Instruction> {
        let program_id = parse_pubkey(&self.trading_program, "Series Trading program")?;
        let accounts = self
            .accounts
            .iter()
            .map(|meta| {
                let key = parse_pubkey(&meta.address, "Series instruction account")?;
                Ok(if meta.writable {
                    solana_program::instruction::AccountMeta::new(key, meta.signer)
                } else {
                    solana_program::instruction::AccountMeta::new_readonly(key, meta.signer)
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Instruction {
            program_id,
            accounts,
            data: decode_base64(&self.data_base64, "Series Hot instruction")?,
        })
    }
}

/// Exact signed packet and resolved address-table projection persisted before
/// the first submission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesTerminalPacketBindingV1 {
    pub(crate) signed: SignedVersionedPacketV1,
    pub(crate) payer: String,
    pub(crate) lookup_table: String,
    pub(crate) lookup_table_sha256: String,
    pub(crate) resolved_account_keys: Vec<String>,
    pub(crate) resolved_account_keys_sha256: String,
    pub(crate) packet_binding_sha256: String,
}

/// Finalized success evidence for one exact packet and poststate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesTerminalFinalizationV1 {
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) packet_sha256: String,
    pub(crate) fee_lamports: u64,
    pub(crate) compute_units_consumed: u64,
    pub(crate) poststate_sha256: String,
    pub(crate) complete_source_credit_lamports: Option<u64>,
    pub(crate) finalization_sha256: String,
}

/// One complete crash-safe action journal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesTerminalJournalV1 {
    pub(crate) schema: String,
    pub(crate) campaign_sha256: String,
    pub(crate) sequence: u32,
    pub(crate) ledger: SeriesLedgerIdentityV1,
    pub(crate) planner_finalized_slot: u64,
    pub(crate) planner_snapshot_sha256: String,
    pub(crate) planner_next_occurrence: u32,
    pub(crate) planner_outstanding_tickets: u32,
    pub(crate) action: SeriesJournalActionV1,
    pub(crate) consequence: String,
    pub(crate) template: String,
    pub(crate) occurrence: Option<String>,
    pub(crate) ticket: Option<String>,
    pub(crate) expected_series_revision: u64,
    pub(crate) expected_ticket_revision: u64,
    pub(crate) request_base64: String,
    pub(crate) request_sha256: String,
    pub(crate) phase: SeriesTerminalJournalPhaseV1,
    physical: Option<DurableSeriesPhysicalActionV1>,
    payer: Option<String>,
    prestate: Option<SeriesChainProjectionV1>,
    poststate: Option<SeriesChainProjectionV1>,
    pub(crate) packet: Option<SeriesTerminalPacketBindingV1>,
    pub(crate) finalization: Option<SeriesTerminalFinalizationV1>,
    pub(crate) intent_sha256: String,
    pub(crate) state_sha256: String,
}

/// Recovery behavior for a durable journal phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SeriesTerminalRecoveryV1 {
    PrepareSelectedGenericHot,
    SignOnceAndPersistDispatching,
    PollThenResendIdentical,
    PollOnly,
    Complete,
}

/// Result of one restart-safe exterior pass. `Pending` always means the exact
/// durable signature remains the only transaction identity the next pass may
/// poll or resend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SeriesTerminalRpcAdvanceV1 {
    Dispatching(SeriesTerminalJournalV1),
    Pending(SeriesTerminalJournalV1),
    Finalized {
        journal: SeriesTerminalJournalV1,
        conservation: Option<SeriesTerminalConservationReceiptV1>,
    },
}

/// Successful terminal transfer proved from finalized pre/post account facts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesTerminalConservationReceiptV1 {
    pub(crate) schema: String,
    pub(crate) campaign_sha256: String,
    pub(crate) ledger_identity_sha256: String,
    pub(crate) action: SeriesJournalActionV1,
    pub(crate) source: String,
    pub(crate) rent_credit: String,
    pub(crate) source_lamports_before: u64,
    pub(crate) rent_credit_lamports_before: u64,
    pub(crate) rent_credit_lamports_after: u64,
    pub(crate) donation_inclusive_exact_credit: bool,
    pub(crate) payer: String,
    pub(crate) fee_lamports: u64,
    pub(crate) receipt_sha256: String,
}

/// Finalized failed transaction plus exact rollback proof.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesTerminalRollbackReceiptV1 {
    pub(crate) schema: String,
    pub(crate) campaign_sha256: String,
    pub(crate) ledger_identity_sha256: String,
    pub(crate) action: SeriesJournalActionV1,
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) packet_sha256: String,
    pub(crate) exact_custom_refusal_code: u32,
    pub(crate) fee_lamports: u64,
    pub(crate) compute_units_consumed: u64,
    pub(crate) protocol_accounts_byte_and_lamport_exact: bool,
    pub(crate) distinct_payer_fee_only: bool,
    pub(crate) prestate_sha256: String,
    pub(crate) poststate_sha256: String,
    pub(crate) receipt_sha256: String,
}

/// Family-neutral aggregate-Market retirement reauthenticated for one Series
/// parent or consumed child Market. The constructor re-runs the current
/// retirement operator and matches all four physical instructions before this
/// compact binding can enter the Series completion ledger.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesMarketRetirementBindingV1 {
    pub(crate) ledger_identity_sha256: String,
    pub(crate) selected_release_set: String,
    pub(crate) market: String,
    pub(crate) generation: u64,
    pub(crate) rent_credit: String,
    pub(crate) core_program: String,
    pub(crate) claims_program: String,
    pub(crate) aggregate_campaign_sha256: String,
    pub(crate) aggregate_completion_sha256: String,
    pub(crate) finalized_slot: u64,
    pub(crate) total_transaction_fees_lamports: u64,
    pub(crate) total_compute_units_consumed: u64,
    pub(crate) binding_sha256: String,
}

/// Reauthenticate one generic AggregateRetirement completion for a Series
/// parent or consumed occurrence Market.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_series_market_retirement_v1(
    ledger: &SeriesLedgerIdentityV1,
    expected_release_set: [u8; 32],
    expected_market: Pubkey,
    expected_generation: u64,
    expected_rent_credit: Pubkey,
    snapshot: &MarketRetirementSnapshotV1,
    campaign: &AggregateRetirementCampaignV1,
    receipt: &AggregateRetirementConservationReceiptV1,
) -> Result<SeriesMarketRetirementBindingV1> {
    authenticate_ledger_identity_v1(ledger)?;
    authenticate_aggregate_retirement_campaign_v1(campaign)?;
    authenticate_aggregate_retirement_conservation_receipt_v1(receipt)?;
    let report = build_checkpoint_market_retirement_v1(snapshot)
        .map_err(|error| refusal(format!("generic Market retirement operator: {error:?}")))?;
    let fresh = [
        &report.prepare,
        &report.close_vault,
        &report.close_replay,
        &report.finish,
    ];
    if campaign.operations.len() != fresh.len() {
        return Err(refusal(
            "Series Market retirement did not reproduce the generic operator's four instructions",
        ));
    }
    for (durable, fresh) in campaign.operations.iter().zip(fresh) {
        if durable.instruction()? != *fresh {
            return Err(refusal(
                "Series Market retirement did not reproduce the generic operator's four instructions",
            ));
        }
    }
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| refusal("Series retirement Market was not a canonical Core state"))?;
    if expected_release_set == [0; 32]
        || expected_generation == 0
        || expected_market != snapshot.market.key
        || expected_market.to_string() != campaign.market.address
        || expected_market.to_string() != receipt.market
        || expected_generation != market.identity.generation
        || expected_release_set != market.identity.selected_release_set.to_bytes()
        || expected_rent_credit != snapshot.rent_credit.key
        || expected_rent_credit.to_string() != campaign.rent_credit.address
        || expected_rent_credit.to_string() != receipt.rent_credit
        || campaign.genesis_hash != ledger.genesis_hash
        || campaign.campaign_sha256 != receipt.campaign_sha256
        || campaign.core_program != snapshot.core_program.key.to_string()
        || campaign.claims_program != snapshot.claims_program.key.to_string()
        || !durable_retirement_account_matches_v1(&campaign.market, &snapshot.market)
        || !durable_retirement_account_matches_v1(&campaign.rent_credit, &snapshot.rent_credit)
    {
        return Err(refusal(
            "Series Market retirement changed release, generation, program, Market, RentCredit, or ledger",
        ));
    }
    let finalized_slot = receipt
        .journals
        .last()
        .map(|journal| journal.finalized_slot)
        .ok_or_else(|| refusal("generic Market retirement omitted final journal"))?;
    let total_compute_units_consumed =
        receipt.journals.iter().try_fold(0_u64, |sum, journal| {
            sum.checked_add(journal.compute_units_consumed)
                .ok_or_else(|| refusal("generic Market retirement compute sum overflowed"))
        })?;
    let mut binding = SeriesMarketRetirementBindingV1 {
        ledger_identity_sha256: ledger.identity_sha256.clone(),
        selected_release_set: hex32(expected_release_set),
        market: expected_market.to_string(),
        generation: expected_generation,
        rent_credit: expected_rent_credit.to_string(),
        core_program: campaign.core_program.clone(),
        claims_program: campaign.claims_program.clone(),
        aggregate_campaign_sha256: campaign.campaign_sha256.clone(),
        aggregate_completion_sha256: receipt.receipt_sha256.clone(),
        finalized_slot,
        total_transaction_fees_lamports: receipt.total_transaction_fees_lamports,
        total_compute_units_consumed,
        binding_sha256: String::new(),
    };
    binding.binding_sha256 = market_retirement_binding_digest_v1(&binding)?;
    authenticate_series_market_retirement_binding_v1(&binding)?;
    Ok(binding)
}

fn durable_retirement_account_matches_v1(
    durable: &crate::aggregate_retirement_journal::DurableRetirementAccountV1,
    observed: &dclutch_market_retirement_v1_operator::ObservedAccount,
) -> bool {
    durable.address == observed.key.to_string()
        && durable.owner == observed.owner.to_string()
        && durable.lamports == observed.lamports
        && durable.executable == observed.executable
        && durable.data_len == observed.data.len()
        && durable.data_sha256 == sha256_hex(&observed.data)
}

pub(crate) fn authenticate_series_market_retirement_binding_v1(
    binding: &SeriesMarketRetirementBindingV1,
) -> Result<()> {
    for digest in [
        &binding.ledger_identity_sha256,
        &binding.selected_release_set,
        &binding.aggregate_campaign_sha256,
        &binding.aggregate_completion_sha256,
    ] {
        require_sha256(digest, "Series Market retirement binding")?;
    }
    if binding
        .selected_release_set
        .bytes()
        .all(|byte| byte == b'0')
        || binding.generation == 0
        || binding.finalized_slot == 0
        || binding.binding_sha256 != market_retirement_binding_digest_v1(binding)?
    {
        return Err(refusal("Series Market retirement binding changed"));
    }
    parse_pubkey(&binding.market, "retired Series Market")?;
    parse_pubkey(&binding.rent_credit, "retired Series Market RentCredit")?;
    parse_pubkey(&binding.core_program, "Series retirement Core program")?;
    parse_pubkey(&binding.claims_program, "Series retirement Claims program")?;
    Ok(())
}

/// Physical Found transaction which established the recurring root on the same
/// ledger. This is a transaction binding, not a second Found checkpoint schema.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesFoundBindingV1 {
    pub(crate) campaign_sha256: String,
    pub(crate) ledger_identity_sha256: String,
    pub(crate) root: String,
    /// Parent Market carrying the recurring-Series capability root.
    pub(crate) parent_market: String,
    /// Exact parent Market generation at Series Found.
    pub(crate) parent_market_generation: u64,
    pub(crate) template: String,
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) packet_sha256: String,
    pub(crate) poststate_sha256: String,
    pub(crate) binding_sha256: String,
}

/// One occurrence reconstructed exclusively from finalized planner journals.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesOccurrenceCompletionV1 {
    pub(crate) occurrence: String,
    pub(crate) ticket: String,
    pub(crate) ticket_account: String,
    pub(crate) prepare_journal_sha256: String,
    pub(crate) prepare_signature: String,
    pub(crate) settlement: SeriesJournalActionV1,
    pub(crate) settlement_journal_sha256: String,
    pub(crate) settlement_signature: String,
    pub(crate) retire_journal_sha256: String,
    pub(crate) retire_signature: String,
    pub(crate) complete_ticket_credit_lamports: u64,
    pub(crate) retirement_conservation_sha256: String,
    /// Canonical distinct future Market selected at Prepare for this occurrence.
    pub(crate) future_market: String,
    /// Canonical nonzero future Market generation.
    pub(crate) future_market_generation: u64,
    /// Exact authenticated permit consumed by Expire; absent for Consume.
    pub(crate) expire_permit: Option<String>,
    /// Exact Expire poststate proving both future Market and permit vacancy.
    pub(crate) expire_vacancy_poststate_sha256: Option<String>,
    /// Real child Market created by Consume; absent for Expire.
    pub(crate) child_market: Option<String>,
    /// Child Market generation; absent for Expire.
    pub(crate) child_market_generation: Option<u64>,
    /// Authenticated generic AggregateRetirement completion for Consume.
    pub(crate) child_market_retirement_sha256: Option<String>,
}

/// Compact finalized journal receipt retained by the machine ledger.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesActionCompletionV1 {
    pub(crate) sequence: u32,
    pub(crate) action: SeriesJournalActionV1,
    pub(crate) request_sha256: String,
    pub(crate) journal_sha256: String,
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) packet_sha256: String,
    pub(crate) poststate_sha256: String,
    pub(crate) fee_lamports: u64,
    pub(crate) compute_units_consumed: u64,
}

/// Native recurring-Series completion ledger. It cannot ingest or relabel a
/// Direct terminal/aggregate completion object because its constructor accepts
/// only authenticated Series journals and Series conservation receipts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SeriesCompleteLifecycleLedgerV1 {
    pub(crate) schema: String,
    pub(crate) status: String,
    pub(crate) complete: bool,
    pub(crate) same_existing_validator_ledger: bool,
    pub(crate) campaign_sha256: String,
    pub(crate) ledger: SeriesLedgerIdentityV1,
    pub(crate) root: String,
    pub(crate) template: String,
    pub(crate) found: SeriesFoundBindingV1,
    pub(crate) occurrence_count: u32,
    pub(crate) consumed_occurrences: u32,
    pub(crate) expired_occurrences: u32,
    pub(crate) tickets_retired_exactly_once: bool,
    pub(crate) root_closed: bool,
    pub(crate) occurrence_completions: Vec<SeriesOccurrenceCompletionV1>,
    pub(crate) actions: Vec<SeriesActionCompletionV1>,
    /// Consumed-child retirements in occurrence order, then parent retirement.
    pub(crate) market_retirements: Vec<SeriesMarketRetirementBindingV1>,
    pub(crate) root_close_conservation_sha256: String,
    pub(crate) parent_market_retirement_sha256: String,
    pub(crate) all_created_markets_retired: bool,
    pub(crate) total_terminal_credit_lamports: u64,
    pub(crate) series_action_transaction_fees_lamports: u64,
    pub(crate) market_retirement_transaction_fees_lamports: u64,
    pub(crate) total_transaction_fees_lamports: u64,
    pub(crate) series_action_compute_units_consumed: u64,
    pub(crate) market_retirement_compute_units_consumed: u64,
    pub(crate) total_compute_units_consumed: u64,
    pub(crate) temporary_protocol_state_closed: bool,
    pub(crate) ledger_sha256: String,
}

struct OpenOccurrenceV1<'a> {
    occurrence: String,
    ticket: String,
    ticket_account: String,
    occurrence_market: String,
    occurrence_market_generation: u64,
    /// Expire-only authenticated permit key. Prepare and Consume deliberately
    /// cannot author or project it before the permit exists.
    occurrence_permit: Option<String>,
    prepare: &'a SeriesTerminalJournalV1,
    settlement: Option<&'a SeriesTerminalJournalV1>,
    retire: Option<(
        &'a SeriesTerminalJournalV1,
        &'a SeriesTerminalConservationReceiptV1,
    )>,
}

/// Bind the already-executed recurring-Series Found transaction to this
/// campaign and existing validator ledger.
#[allow(clippy::too_many_arguments)]
pub(crate) fn admit_series_found_binding_v1(
    campaign_sha256: String,
    ledger: &SeriesLedgerIdentityV1,
    root: Pubkey,
    parent_market: Pubkey,
    parent_market_generation: u64,
    template: [u8; 32],
    signature: String,
    finalized_slot: u64,
    packet_sha256: String,
    poststate_sha256: String,
) -> Result<SeriesFoundBindingV1> {
    authenticate_ledger_identity_v1(ledger)?;
    let mut value = SeriesFoundBindingV1 {
        campaign_sha256,
        ledger_identity_sha256: ledger.identity_sha256.clone(),
        root: root.to_string(),
        parent_market: parent_market.to_string(),
        parent_market_generation,
        template: hex32(template),
        signature,
        finalized_slot,
        packet_sha256,
        poststate_sha256,
        binding_sha256: String::new(),
    };
    value.binding_sha256 = found_binding_digest_v1(&value)?;
    authenticate_found_binding_v1(&value)?;
    Ok(value)
}

/// Converge Found, more than one occurrence, both Consume and Expire,
/// once-only Ticket funding closes, and root lifecycle Close into one native
/// Series machine ledger.
pub(crate) fn build_series_complete_lifecycle_ledger_v1(
    found: SeriesFoundBindingV1,
    ledger: SeriesLedgerIdentityV1,
    journals: &[SeriesTerminalJournalV1],
    conservation: &[SeriesTerminalConservationReceiptV1],
    market_retirements: &[SeriesMarketRetirementBindingV1],
) -> Result<SeriesCompleteLifecycleLedgerV1> {
    authenticate_found_binding_v1(&found)?;
    authenticate_ledger_identity_v1(&ledger)?;
    if found.ledger_identity_sha256 != ledger.identity_sha256 || journals.is_empty() {
        return Err(refusal(
            "Series completion did not remain on its existing Found ledger",
        ));
    }

    let mut conservation_by_source = BTreeMap::new();
    for receipt in conservation {
        authenticate_conservation_receipt_v1(receipt)?;
        if receipt.campaign_sha256 != found.campaign_sha256
            || receipt.ledger_identity_sha256 != ledger.identity_sha256
            || conservation_by_source
                .insert(receipt.source.clone(), receipt)
                .is_some()
        {
            return Err(refusal(
                "Series terminal conservation changed campaign, ledger, or source uniqueness",
            ));
        }
    }

    let mut occurrences = Vec::<OpenOccurrenceV1<'_>>::new();
    let mut ticket_to_occurrence = BTreeMap::<String, usize>::new();
    let mut ticket_accounts = BTreeSet::new();
    let mut actions = Vec::with_capacity(journals.len());
    let mut signatures = BTreeSet::new();
    let mut packets = BTreeSet::new();
    let mut used_conservation = BTreeSet::new();
    let mut last_seen_accounts = BTreeMap::<String, DurableSeriesAccountV1>::new();
    let mut expected_next_occurrence = 0_u32;
    let mut expected_outstanding = 0_u32;
    let mut prior_slot = found.finalized_slot;
    let mut common_authority = None::<String>;
    let mut common_release = None::<String>;
    let mut terminal_credit = None::<String>;
    let mut close = None::<(
        &SeriesTerminalJournalV1,
        &SeriesTerminalConservationReceiptV1,
    )>;
    let mut total_fees = 0_u64;
    let mut total_compute = 0_u64;

    for (index, journal) in journals.iter().enumerate() {
        authenticate_series_terminal_journal_v1(journal)?;
        let expected_sequence = u32::try_from(index)
            .map_err(|_| refusal("Series completion journal count exceeded u32"))?;
        let finalization = journal
            .finalization
            .as_ref()
            .ok_or_else(|| refusal("Series completion included an unfinalized journal"))?;
        let physical = journal
            .physical
            .as_ref()
            .ok_or_else(|| refusal("Series completion journal omitted selected physical action"))?;
        let prestate = journal
            .prestate
            .as_ref()
            .ok_or_else(|| refusal("Series completion journal omitted exact prestate"))?;
        let poststate = journal
            .poststate
            .as_ref()
            .ok_or_else(|| refusal("Series completion journal omitted exact poststate"))?;
        if journal.phase != SeriesTerminalJournalPhaseV1::Finalized
            || journal.sequence != expected_sequence
            || journal.campaign_sha256 != found.campaign_sha256
            || journal.ledger.identity_sha256 != ledger.identity_sha256
            || journal.template != found.template
            || physical.root != found.root
            || physical.parent_market != found.parent_market
            || physical.parent_market_generation != found.parent_market_generation
            || journal.planner_finalized_slot < prior_slot
            || finalization.finalized_slot < prior_slot
            || !signatures.insert(finalization.signature.clone())
            || !packets.insert(finalization.packet_sha256.clone())
            || journal.planner_next_occurrence != expected_next_occurrence
            || journal.planner_outstanding_tickets != expected_outstanding
        {
            return Err(refusal(
                "Series completion changed order, authority, replay counters, or transaction identity",
            ));
        }
        for (key, account) in &prestate.accounts {
            if last_seen_accounts
                .get(key)
                .is_some_and(|previous| previous != account)
            {
                return Err(refusal(
                    "Series journal prestate did not continue the last observed finalized account",
                ));
            }
        }
        prior_slot = finalization.finalized_slot;
        match common_authority.as_ref() {
            Some(expected) if expected != &physical.authority.authority_sha256 => {
                return Err(refusal(
                    "Series completion crossed selected release authority",
                ));
            }
            None => common_authority = Some(physical.authority.authority_sha256.clone()),
            Some(_) => {}
        }
        match common_release.as_ref() {
            Some(expected) if expected != &physical.authority.release_set => {
                return Err(refusal(
                    "Series completion crossed its selected release set",
                ));
            }
            None => common_release = Some(physical.authority.release_set.clone()),
            Some(_) => {}
        }
        total_fees = total_fees
            .checked_add(finalization.fee_lamports)
            .ok_or_else(|| refusal("Series completion fee sum overflowed"))?;
        total_compute = total_compute
            .checked_add(finalization.compute_units_consumed)
            .ok_or_else(|| refusal("Series completion compute sum overflowed"))?;
        actions.push(SeriesActionCompletionV1 {
            sequence: journal.sequence,
            action: journal.action,
            request_sha256: journal.request_sha256.clone(),
            journal_sha256: journal.state_sha256.clone(),
            signature: finalization.signature.clone(),
            finalized_slot: finalization.finalized_slot,
            packet_sha256: finalization.packet_sha256.clone(),
            poststate_sha256: finalization.poststate_sha256.clone(),
            fee_lamports: finalization.fee_lamports,
            compute_units_consumed: finalization.compute_units_consumed,
        });

        match journal.action {
            SeriesJournalActionV1::Prepare => {
                if close.is_some() {
                    return Err(refusal("Series Prepare followed terminal root Close"));
                }
                let occurrence = journal
                    .occurrence
                    .clone()
                    .ok_or_else(|| refusal("Series Prepare omitted occurrence content"))?;
                let ticket = journal
                    .ticket
                    .clone()
                    .ok_or_else(|| refusal("Series Prepare omitted Ticket content"))?;
                let ticket_account = physical
                    .ticket
                    .clone()
                    .ok_or_else(|| refusal("Series Prepare omitted physical Ticket"))?;
                let occurrence_market = physical
                    .occurrence_market
                    .clone()
                    .ok_or_else(|| refusal("Series Prepare omitted canonical future Market"))?;
                let occurrence_market_generation = physical
                    .occurrence_market_generation
                    .ok_or_else(|| refusal("Series Prepare omitted future Market generation"))?;
                if ticket_to_occurrence.contains_key(&ticket)
                    || occurrences
                        .iter()
                        .any(|candidate| candidate.occurrence == occurrence)
                    || !ticket_accounts.insert(ticket_account.clone())
                {
                    return Err(refusal(
                        "Series occurrence or Ticket was prepared more than once",
                    ));
                }
                let occurrence_index = occurrences.len();
                ticket_to_occurrence.insert(ticket.clone(), occurrence_index);
                occurrences.push(OpenOccurrenceV1 {
                    occurrence,
                    ticket,
                    ticket_account,
                    occurrence_market,
                    occurrence_market_generation,
                    occurrence_permit: None,
                    prepare: journal,
                    settlement: None,
                    retire: None,
                });
                expected_outstanding = expected_outstanding
                    .checked_add(1)
                    .ok_or_else(|| refusal("Series outstanding Ticket count overflowed"))?;
            }
            SeriesJournalActionV1::Consume | SeriesJournalActionV1::Expire => {
                if close.is_some() {
                    return Err(refusal("Series settlement followed terminal root Close"));
                }
                let ticket = journal
                    .ticket
                    .as_ref()
                    .ok_or_else(|| refusal("Series settlement omitted Ticket content"))?;
                let occurrence_index = *ticket_to_occurrence
                    .get(ticket)
                    .ok_or_else(|| refusal("Series settlement had no prior Prepare"))?;
                let candidate = occurrences
                    .get_mut(occurrence_index)
                    .ok_or_else(|| refusal("Series settlement occurrence index escaped"))?;
                if candidate.settlement.is_some()
                    || journal.occurrence.as_ref() != Some(&candidate.occurrence)
                    || physical.ticket.as_ref() != Some(&candidate.ticket_account)
                    || physical.occurrence_market.as_ref() != Some(&candidate.occurrence_market)
                    || physical.occurrence_market_generation
                        != Some(candidate.occurrence_market_generation)
                {
                    return Err(refusal(
                        "Series occurrence settlement changed or replayed its Prepare",
                    ));
                }
                let market_post = require_account(
                    poststate,
                    &candidate.occurrence_market,
                    "Series occurrence Market",
                )?;
                match journal.action {
                    SeriesJournalActionV1::Consume => {
                        if !market_post.present || physical.occurrence_permit.is_some() {
                            return Err(refusal(
                                "Series Consume did not found its child Market or projected a pre-Found permit",
                            ));
                        }
                    }
                    SeriesJournalActionV1::Expire => {
                        let permit = physical.occurrence_permit.as_ref().ok_or_else(|| {
                            refusal("Series Expire omitted its authenticated permit")
                        })?;
                        let permit_post =
                            require_account(poststate, permit, "Series occurrence permit")?;
                        if market_post.present || permit_post.present {
                            return Err(refusal(
                                "Series Expire left its future Market or authenticated permit present",
                            ));
                        }
                        candidate.occurrence_permit = Some(permit.clone());
                    }
                    _ => unreachable!("settlement branch admits only Consume or Expire"),
                }
                candidate.settlement = Some(journal);
                expected_next_occurrence = expected_next_occurrence
                    .checked_add(1)
                    .ok_or_else(|| refusal("Series occurrence counter overflowed"))?;
            }
            SeriesJournalActionV1::Retire => {
                if close.is_some() {
                    return Err(refusal("Series Ticket retirement followed root Close"));
                }
                let ticket = journal
                    .ticket
                    .as_ref()
                    .ok_or_else(|| refusal("Series Retire omitted Ticket content"))?;
                let occurrence_index = *ticket_to_occurrence
                    .get(ticket)
                    .ok_or_else(|| refusal("Series Retire had no prior Prepare"))?;
                let candidate = occurrences
                    .get_mut(occurrence_index)
                    .ok_or_else(|| refusal("Series Retire occurrence index escaped"))?;
                let ticket_account = physical
                    .ticket
                    .as_ref()
                    .ok_or_else(|| refusal("Series Retire omitted physical Ticket"))?;
                let receipt = conservation_by_source
                    .get(ticket_account)
                    .copied()
                    .ok_or_else(|| refusal("Series Retire omitted Ticket conservation"))?;
                if candidate.settlement.is_none()
                    || candidate.retire.is_some()
                    || ticket_account != &candidate.ticket_account
                    || receipt.action != SeriesJournalActionV1::Retire
                    || receipt.fee_lamports != finalization.fee_lamports
                    || receipt.payer != prepared_payer_v1(journal)?
                    || !used_conservation.insert(receipt.receipt_sha256.clone())
                {
                    return Err(refusal(
                        "Series Ticket funding Close was missing, replayed, or mismatched",
                    ));
                }
                require_same_terminal_credit_v1(&mut terminal_credit, receipt)?;
                candidate.retire = Some((journal, receipt));
                expected_outstanding = expected_outstanding
                    .checked_sub(1)
                    .ok_or_else(|| refusal("Series Ticket retirement underflowed outstanding"))?;
            }
            SeriesJournalActionV1::Close => {
                let receipt = conservation_by_source
                    .get(&physical.root)
                    .copied()
                    .ok_or_else(|| refusal("Series Close omitted root conservation"))?;
                if index + 1 != journals.len()
                    || close.is_some()
                    || expected_outstanding != 0
                    || occurrences.iter().any(|candidate| {
                        candidate.settlement.is_none() || candidate.retire.is_none()
                    })
                    || receipt.action != SeriesJournalActionV1::Close
                    || receipt.fee_lamports != finalization.fee_lamports
                    || receipt.payer != prepared_payer_v1(journal)?
                    || !used_conservation.insert(receipt.receipt_sha256.clone())
                {
                    return Err(refusal(
                        "Series root Close preceded Ticket closure or changed its conservation",
                    ));
                }
                require_same_terminal_credit_v1(&mut terminal_credit, receipt)?;
                close = Some((journal, receipt));
            }
        }
        for (key, account) in &poststate.accounts {
            last_seen_accounts.insert(key.clone(), account.clone());
        }
    }

    finish_series_complete_lifecycle_v1(
        found,
        ledger,
        occurrences,
        actions,
        conservation,
        market_retirements,
        used_conservation,
        close,
        common_release.ok_or_else(|| refusal("Series completion omitted selected release"))?,
        total_fees,
        total_compute,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_series_complete_lifecycle_v1(
    found: SeriesFoundBindingV1,
    ledger: SeriesLedgerIdentityV1,
    occurrences: Vec<OpenOccurrenceV1<'_>>,
    actions: Vec<SeriesActionCompletionV1>,
    conservation: &[SeriesTerminalConservationReceiptV1],
    market_retirements: &[SeriesMarketRetirementBindingV1],
    used_conservation: BTreeSet<String>,
    close: Option<(
        &SeriesTerminalJournalV1,
        &SeriesTerminalConservationReceiptV1,
    )>,
    selected_release: String,
    total_fees: u64,
    total_compute: u64,
) -> Result<SeriesCompleteLifecycleLedgerV1> {
    let (close_journal, close_receipt) =
        close.ok_or_else(|| refusal("Series completion omitted terminal root Close"))?;
    let consumed_occurrences = occurrences
        .iter()
        .filter(|value| {
            value
                .settlement
                .is_some_and(|journal| journal.action == SeriesJournalActionV1::Consume)
        })
        .count();
    let expired_occurrences = occurrences
        .iter()
        .filter(|value| {
            value
                .settlement
                .is_some_and(|journal| journal.action == SeriesJournalActionV1::Expire)
        })
        .count();
    if occurrences.len() < 2
        || consumed_occurrences == 0
        || expired_occurrences == 0
        || used_conservation.len() != conservation.len()
        || market_retirements.len() != consumed_occurrences.saturating_add(1)
    {
        return Err(refusal(
            "Series completion requires two occurrences, Consume, Expire, exact Market retirements, and no unused conservation",
        ));
    }
    let mut retirement_iter = market_retirements.iter();
    let mut retired_markets = BTreeSet::new();
    let mut retirement_campaigns = BTreeSet::new();
    let mut retirement_completions = BTreeSet::new();
    let occurrence_completions = occurrences
        .iter()
        .map(|value| {
            let settlement = value
                .settlement
                .ok_or_else(|| refusal("Series occurrence omitted settlement"))?;
            let (retire, receipt) = value
                .retire
                .ok_or_else(|| refusal("Series occurrence omitted Ticket retirement"))?;
            let prepare_final = value
                .prepare
                .finalization
                .as_ref()
                .ok_or_else(|| refusal("Series Prepare omitted finalization"))?;
            let settlement_final = settlement
                .finalization
                .as_ref()
                .ok_or_else(|| refusal("Series settlement omitted finalization"))?;
            let retire_final = retire
                .finalization
                .as_ref()
                .ok_or_else(|| refusal("Series Retire omitted finalization"))?;
            let (child_market, child_market_generation, child_market_retirement_sha256) =
                if settlement.action == SeriesJournalActionV1::Consume {
                    let retirement = retirement_iter.next().ok_or_else(|| {
                        refusal("consumed Series occurrence omitted child Market retirement")
                    })?;
                    authenticate_series_market_retirement_binding_v1(retirement)?;
                    if retirement.ledger_identity_sha256 != ledger.identity_sha256
                        || retirement.selected_release_set != selected_release
                        || retirement.market != value.occurrence_market
                        || retirement.generation != value.occurrence_market_generation
                        || retirement.finalized_slot < settlement_final.finalized_slot
                        || !retired_markets.insert(retirement.market.clone())
                        || !retirement_campaigns
                            .insert(retirement.aggregate_campaign_sha256.clone())
                        || !retirement_completions
                            .insert(retirement.aggregate_completion_sha256.clone())
                    {
                        return Err(refusal(
                            "consumed Series child Market retirement changed release, generation, order, or identity",
                        ));
                    }
                    (
                        Some(value.occurrence_market.clone()),
                        Some(value.occurrence_market_generation),
                        Some(retirement.binding_sha256.clone()),
                    )
                } else {
                    (None, None, None)
                };
            Ok(SeriesOccurrenceCompletionV1 {
                occurrence: value.occurrence.clone(),
                ticket: value.ticket.clone(),
                ticket_account: value.ticket_account.clone(),
                prepare_journal_sha256: value.prepare.state_sha256.clone(),
                prepare_signature: prepare_final.signature.clone(),
                settlement: settlement.action,
                settlement_journal_sha256: settlement.state_sha256.clone(),
                settlement_signature: settlement_final.signature.clone(),
                retire_journal_sha256: retire.state_sha256.clone(),
                retire_signature: retire_final.signature.clone(),
                complete_ticket_credit_lamports: receipt.source_lamports_before,
                retirement_conservation_sha256: receipt.receipt_sha256.clone(),
                future_market: value.occurrence_market.clone(),
                future_market_generation: value.occurrence_market_generation,
                expire_permit: value.occurrence_permit.clone(),
                expire_vacancy_poststate_sha256: (settlement.action
                    == SeriesJournalActionV1::Expire)
                    .then(|| settlement_final.poststate_sha256.clone()),
                child_market,
                child_market_generation,
                child_market_retirement_sha256,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let parent_retirement = retirement_iter
        .next()
        .ok_or_else(|| refusal("Series completion omitted parent Market retirement"))?;
    authenticate_series_market_retirement_binding_v1(parent_retirement)?;
    let close_finalized_slot = close_journal
        .finalization
        .as_ref()
        .ok_or_else(|| refusal("Series Close omitted finalization"))?
        .finalized_slot;
    if retirement_iter.next().is_some()
        || parent_retirement.ledger_identity_sha256 != ledger.identity_sha256
        || parent_retirement.selected_release_set != selected_release
        || parent_retirement.market != found.parent_market
        || parent_retirement.generation != found.parent_market_generation
        || parent_retirement.finalized_slot < close_finalized_slot
        || !retired_markets.insert(parent_retirement.market.clone())
        || !retirement_campaigns.insert(parent_retirement.aggregate_campaign_sha256.clone())
        || !retirement_completions.insert(parent_retirement.aggregate_completion_sha256.clone())
    {
        return Err(refusal(
            "Series parent Market retirement changed release, generation, order, or identity",
        ));
    }
    let total_terminal_credit_lamports = conservation.iter().try_fold(0_u64, |sum, receipt| {
        sum.checked_add(receipt.source_lamports_before)
            .ok_or_else(|| refusal("Series terminal credit sum overflowed"))
    })?;
    let market_retirement_transaction_fees_lamports =
        market_retirements.iter().try_fold(0_u64, |sum, binding| {
            sum.checked_add(binding.total_transaction_fees_lamports)
                .ok_or_else(|| refusal("Series Market retirement fee sum overflowed"))
        })?;
    let market_retirement_compute_units_consumed =
        market_retirements.iter().try_fold(0_u64, |sum, binding| {
            sum.checked_add(binding.total_compute_units_consumed)
                .ok_or_else(|| refusal("Series Market retirement compute sum overflowed"))
        })?;
    let all_transaction_fees = total_fees
        .checked_add(market_retirement_transaction_fees_lamports)
        .ok_or_else(|| refusal("Series complete fee sum overflowed"))?;
    let all_compute_units = total_compute
        .checked_add(market_retirement_compute_units_consumed)
        .ok_or_else(|| refusal("Series complete compute sum overflowed"))?;
    let occurrence_count = u32::try_from(occurrences.len())
        .map_err(|_| refusal("Series occurrence count exceeded u32"))?;
    let mut completion = SeriesCompleteLifecycleLedgerV1 {
        schema: SERIES_COMPLETE_LIFECYCLE_SCHEMA_V1.into(),
        status: "finalized".into(),
        complete: true,
        same_existing_validator_ledger: true,
        campaign_sha256: found.campaign_sha256.clone(),
        ledger,
        root: found.root.clone(),
        template: found.template.clone(),
        found,
        occurrence_count,
        consumed_occurrences: u32::try_from(consumed_occurrences)
            .map_err(|_| refusal("consumed occurrence count exceeded u32"))?,
        expired_occurrences: u32::try_from(expired_occurrences)
            .map_err(|_| refusal("expired occurrence count exceeded u32"))?,
        tickets_retired_exactly_once: true,
        root_closed: true,
        occurrence_completions,
        actions,
        market_retirements: market_retirements.to_vec(),
        root_close_conservation_sha256: close_receipt.receipt_sha256.clone(),
        parent_market_retirement_sha256: parent_retirement.binding_sha256.clone(),
        all_created_markets_retired: true,
        total_terminal_credit_lamports,
        series_action_transaction_fees_lamports: total_fees,
        market_retirement_transaction_fees_lamports,
        total_transaction_fees_lamports: all_transaction_fees,
        series_action_compute_units_consumed: total_compute,
        market_retirement_compute_units_consumed,
        total_compute_units_consumed: all_compute_units,
        temporary_protocol_state_closed: true,
        ledger_sha256: String::new(),
    };
    if close_journal.physical.as_ref().map(|value| &value.root) != Some(&completion.root) {
        return Err(refusal(
            "Series completion root Close changed the Found root",
        ));
    }
    completion.ledger_sha256 = complete_lifecycle_digest_v1(&completion)?;
    authenticate_series_complete_lifecycle_ledger_v1(&completion)?;
    Ok(completion)
}

pub(crate) fn authenticate_series_complete_lifecycle_ledger_v1(
    completion: &SeriesCompleteLifecycleLedgerV1,
) -> Result<()> {
    authenticate_ledger_identity_v1(&completion.ledger)?;
    authenticate_found_binding_v1(&completion.found)?;
    let consumed = completion
        .occurrence_completions
        .iter()
        .filter(|value| value.settlement == SeriesJournalActionV1::Consume)
        .count();
    let expired = completion
        .occurrence_completions
        .iter()
        .filter(|value| value.settlement == SeriesJournalActionV1::Expire)
        .count();
    let action_fees = completion.actions.iter().try_fold(0_u64, |sum, action| {
        sum.checked_add(action.fee_lamports)
            .ok_or_else(|| refusal("Series completion fee authentication overflowed"))
    })?;
    let action_compute = completion.actions.iter().try_fold(0_u64, |sum, action| {
        sum.checked_add(action.compute_units_consumed)
            .ok_or_else(|| refusal("Series completion compute authentication overflowed"))
    })?;
    let market_fees = completion
        .market_retirements
        .iter()
        .try_fold(0_u64, |sum, binding| {
            sum.checked_add(binding.total_transaction_fees_lamports)
                .ok_or_else(|| refusal("Series retirement fee authentication overflowed"))
        })?;
    let market_compute = completion
        .market_retirements
        .iter()
        .try_fold(0_u64, |sum, binding| {
            sum.checked_add(binding.total_compute_units_consumed)
                .ok_or_else(|| refusal("Series retirement compute authentication overflowed"))
        })?;
    if completion.schema != SERIES_COMPLETE_LIFECYCLE_SCHEMA_V1
        || completion.status != "finalized"
        || !completion.complete
        || !completion.same_existing_validator_ledger
        || !completion.tickets_retired_exactly_once
        || !completion.root_closed
        || !completion.all_created_markets_retired
        || !completion.temporary_protocol_state_closed
        || completion.campaign_sha256 != completion.found.campaign_sha256
        || completion.ledger.identity_sha256 != completion.found.ledger_identity_sha256
        || completion.root != completion.found.root
        || completion.template != completion.found.template
        || usize::try_from(completion.occurrence_count).ok()
            != Some(completion.occurrence_completions.len())
        || completion.occurrence_count < 2
        || consumed == 0
        || expired == 0
        || usize::try_from(completion.consumed_occurrences).ok() != Some(consumed)
        || usize::try_from(completion.expired_occurrences).ok() != Some(expired)
        || completion
            .actions
            .iter()
            .enumerate()
            .any(|(index, action)| usize::try_from(action.sequence).ok() != Some(index))
        || completion.actions.last().map(|value| value.action) != Some(SeriesJournalActionV1::Close)
        || action_fees != completion.series_action_transaction_fees_lamports
        || market_fees != completion.market_retirement_transaction_fees_lamports
        || action_fees.checked_add(market_fees) != Some(completion.total_transaction_fees_lamports)
        || action_compute != completion.series_action_compute_units_consumed
        || market_compute != completion.market_retirement_compute_units_consumed
        || action_compute.checked_add(market_compute)
            != Some(completion.total_compute_units_consumed)
        || completion.ledger_sha256 != complete_lifecycle_digest_v1(completion)?
    {
        return Err(refusal(
            "native Series completion ledger changed or became incomplete",
        ));
    }
    let mut occurrence_ids = BTreeSet::new();
    let mut ticket_ids = BTreeSet::new();
    let mut ticket_accounts = BTreeSet::new();
    let mut future_markets = BTreeSet::new();
    let mut journal_digests = BTreeSet::new();
    let mut signatures = BTreeSet::new();
    let mut market_retirements = completion.market_retirements.iter();
    let mut retired_markets = BTreeSet::new();
    let mut retirement_campaigns = BTreeSet::new();
    let mut retirement_completions = BTreeSet::new();
    let mut selected_release = None::<&str>;
    for occurrence in &completion.occurrence_completions {
        if !occurrence_ids.insert(&occurrence.occurrence)
            || !ticket_ids.insert(&occurrence.ticket)
            || !ticket_accounts.insert(&occurrence.ticket_account)
            || !journal_digests.insert(&occurrence.prepare_journal_sha256)
            || !journal_digests.insert(&occurrence.settlement_journal_sha256)
            || !journal_digests.insert(&occurrence.retire_journal_sha256)
            || !signatures.insert(&occurrence.prepare_signature)
            || !signatures.insert(&occurrence.settlement_signature)
            || !signatures.insert(&occurrence.retire_signature)
        {
            return Err(refusal("Series completion replayed occurrence evidence"));
        }
        for digest in [
            &occurrence.prepare_journal_sha256,
            &occurrence.settlement_journal_sha256,
            &occurrence.retire_journal_sha256,
            &occurrence.retirement_conservation_sha256,
        ] {
            require_sha256(digest, "Series occurrence completion")?;
        }
        let action_for = |digest: &str| {
            completion
                .actions
                .iter()
                .find(|action| action.journal_sha256 == digest)
        };
        let prepare_action = action_for(&occurrence.prepare_journal_sha256)
            .ok_or_else(|| refusal("Series occurrence omitted its Prepare action receipt"))?;
        let settlement_action = action_for(&occurrence.settlement_journal_sha256)
            .ok_or_else(|| refusal("Series occurrence omitted its settlement action receipt"))?;
        let retire_action = action_for(&occurrence.retire_journal_sha256)
            .ok_or_else(|| refusal("Series occurrence omitted its Retire action receipt"))?;
        if prepare_action.action != SeriesJournalActionV1::Prepare
            || prepare_action.signature != occurrence.prepare_signature
            || settlement_action.action != occurrence.settlement
            || settlement_action.signature != occurrence.settlement_signature
            || retire_action.action != SeriesJournalActionV1::Retire
            || retire_action.signature != occurrence.retire_signature
            || occurrence.future_market_generation == 0
            || occurrence.future_market == completion.found.parent_market
            || occurrence.future_market == completion.root
            || !future_markets.insert(&occurrence.future_market)
        {
            return Err(refusal(
                "Series occurrence action links, future Market, or controller separation changed",
            ));
        }
        parse_pubkey(&occurrence.future_market, "Series occurrence future Market")?;
        match occurrence.settlement {
            SeriesJournalActionV1::Consume => {
                let retirement = market_retirements.next().ok_or_else(|| {
                    refusal("Series completion omitted consumed child Market retirement")
                })?;
                authenticate_series_market_retirement_binding_v1(retirement)?;
                let child_market = occurrence
                    .child_market
                    .as_ref()
                    .ok_or_else(|| refusal("consumed Series occurrence omitted child Market"))?;
                let child_generation = occurrence.child_market_generation.ok_or_else(|| {
                    refusal("consumed Series occurrence omitted child Market generation")
                })?;
                let child_retirement = occurrence
                    .child_market_retirement_sha256
                    .as_ref()
                    .ok_or_else(|| {
                        refusal("consumed Series occurrence omitted child retirement binding")
                    })?;
                require_sha256(child_retirement, "Series child Market retirement")?;
                if retirement.ledger_identity_sha256 != completion.ledger.identity_sha256
                    || retirement.market != *child_market
                    || retirement.generation != child_generation
                    || occurrence.future_market != *child_market
                    || occurrence.future_market_generation != child_generation
                    || occurrence.expire_permit.is_some()
                    || occurrence.expire_vacancy_poststate_sha256.is_some()
                    || retirement.binding_sha256 != *child_retirement
                    || !retired_markets.insert(&retirement.market)
                    || !retirement_campaigns.insert(&retirement.aggregate_campaign_sha256)
                    || !retirement_completions.insert(&retirement.aggregate_completion_sha256)
                {
                    return Err(refusal(
                        "Series child Market retirement replayed or changed its exact binding",
                    ));
                }
                match selected_release {
                    Some(expected) if expected != retirement.selected_release_set => {
                        return Err(refusal(
                            "Series Market retirements crossed selected releases",
                        ));
                    }
                    None => selected_release = Some(&retirement.selected_release_set),
                    Some(_) => {}
                }
            }
            SeriesJournalActionV1::Expire => {
                let permit = occurrence
                    .expire_permit
                    .as_ref()
                    .ok_or_else(|| refusal("expired Series occurrence omitted its permit"))?;
                let vacancy = occurrence
                    .expire_vacancy_poststate_sha256
                    .as_ref()
                    .ok_or_else(|| refusal("expired Series occurrence omitted vacancy evidence"))?;
                parse_pubkey(permit, "Series Expire permit")?;
                require_sha256(vacancy, "Series Expire vacancy poststate")?;
                if permit == &occurrence.future_market
                    || permit == &completion.found.parent_market
                    || permit == &completion.root
                    || vacancy != &settlement_action.poststate_sha256
                    || occurrence.child_market.is_some()
                    || occurrence.child_market_generation.is_some()
                    || occurrence.child_market_retirement_sha256.is_some()
                {
                    return Err(refusal(
                        "expired Series occurrence changed its future-Market vacancy or fabricated a child retirement",
                    ));
                }
            }
            _ => {
                return Err(refusal(
                    "Series occurrence completion used a non-settlement action",
                ));
            }
        }
    }
    let parent = market_retirements
        .next()
        .ok_or_else(|| refusal("Series completion omitted parent Market retirement"))?;
    authenticate_series_market_retirement_binding_v1(parent)?;
    if market_retirements.next().is_some()
        || parent.ledger_identity_sha256 != completion.ledger.identity_sha256
        || parent.market != completion.found.parent_market
        || parent.generation != completion.found.parent_market_generation
        || parent.binding_sha256 != completion.parent_market_retirement_sha256
        || selected_release != Some(parent.selected_release_set.as_str())
        || !retired_markets.insert(&parent.market)
        || !retirement_campaigns.insert(&parent.aggregate_campaign_sha256)
        || !retirement_completions.insert(&parent.aggregate_completion_sha256)
    {
        return Err(refusal(
            "Series parent Market retirement replayed or changed its exact binding",
        ));
    }
    require_sha256(
        &completion.root_close_conservation_sha256,
        "Series root Close conservation",
    )?;
    require_sha256(
        &completion.parent_market_retirement_sha256,
        "Series parent Market retirement",
    )?;
    Ok(())
}

fn require_same_terminal_credit_v1(
    expected: &mut Option<String>,
    receipt: &SeriesTerminalConservationReceiptV1,
) -> Result<()> {
    match expected {
        Some(value) if value != &receipt.rent_credit => Err(refusal(
            "Series terminal actions selected different lifecycle RentCredits",
        )),
        None => {
            *expected = Some(receipt.rent_credit.clone());
            Ok(())
        }
        Some(_) => Ok(()),
    }
}

fn authenticate_found_binding_v1(found: &SeriesFoundBindingV1) -> Result<()> {
    require_sha256(&found.campaign_sha256, "Series Found campaign")?;
    require_sha256(&found.ledger_identity_sha256, "Series Found ledger")?;
    require_sha256(&found.template, "Series Found Template")?;
    require_sha256(&found.packet_sha256, "Series Found packet")?;
    require_sha256(&found.poststate_sha256, "Series Found poststate")?;
    parse_pubkey(&found.root, "Series Found root")?;
    parse_pubkey(&found.parent_market, "Series Found parent Market")?;
    if found.parent_market_generation == 0
        || found.parent_market == found.root
        || found.finalized_slot == 0
        || found.signature.parse::<Signature>().is_err()
        || found.binding_sha256 != found_binding_digest_v1(found)?
    {
        return Err(refusal("Series Found transaction binding changed"));
    }
    Ok(())
}

/// Plan the exact next Series act selected by the lifecycle inspector.
pub(crate) fn plan_series_terminal_journal_v1(
    observation: SeriesPlannerObservationV1,
    sequence: u32,
    report: &SeriesLifecycleReportV3,
) -> Result<SeriesTerminalJournalV1> {
    let planned = match report.next() {
        SeriesNextActV3::Ready(planned) => planned,
        SeriesNextActV3::WaitUntil { .. } => {
            return Err(refusal("lifecycle planner selected WaitUntil, not an act"));
        }
        SeriesNextActV3::Acquire(_) => {
            return Err(refusal(
                "lifecycle planner requires more finalized evidence before an act",
            ));
        }
    };
    plan_ready_series_journal_v1(
        observation,
        sequence,
        report.next_occurrence(),
        report.outstanding_tickets(),
        planned,
    )
}

fn plan_ready_series_journal_v1(
    observation: SeriesPlannerObservationV1,
    sequence: u32,
    next_occurrence: u32,
    outstanding_tickets: u32,
    planned: PlannedSeriesActV3,
) -> Result<SeriesTerminalJournalV1> {
    let request = planned.request();
    plan_decoded_series_journal_v1(
        observation,
        sequence,
        next_occurrence,
        outstanding_tickets,
        planned.action(),
        planned.consequence(),
        request.as_bytes(),
    )
}

#[allow(clippy::too_many_arguments)]
fn plan_decoded_series_journal_v1(
    observation: SeriesPlannerObservationV1,
    sequence: u32,
    next_occurrence: u32,
    outstanding_tickets: u32,
    action: SeriesActionV3,
    consequence: SeriesConsequenceV3,
    request_bytes: &[u8],
) -> Result<SeriesTerminalJournalV1> {
    authenticate_ledger_identity_v1(&observation.ledger)?;
    require_sha256(&observation.campaign_sha256, "Series campaign")?;
    require_sha256(&observation.snapshot_sha256, "Series planner snapshot")?;
    if observation.finalized_slot == 0 {
        return Err(refusal("planner observation was not finalized"));
    }
    let request = SeriesActionRequestV3::decode(request_bytes)
        .map_err(|_| refusal("planner returned a noncanonical Series request"))?;
    if request.action() != action || consequence_for_action(action) != consequence {
        return Err(refusal(
            "planner action, consequence, and request did not agree",
        ));
    }
    let mut journal = SeriesTerminalJournalV1 {
        schema: SERIES_TERMINAL_JOURNAL_SCHEMA_V1.into(),
        campaign_sha256: observation.campaign_sha256,
        sequence,
        ledger: observation.ledger,
        planner_finalized_slot: observation.finalized_slot,
        planner_snapshot_sha256: observation.snapshot_sha256,
        planner_next_occurrence: next_occurrence,
        planner_outstanding_tickets: outstanding_tickets,
        action: SeriesJournalActionV1::from_kernel(action),
        consequence: consequence_text(consequence).into(),
        template: hex32(request.template().to_bytes()),
        occurrence: request.occurrence().map(|value| hex32(value.to_bytes())),
        ticket: request.ticket().map(|value| hex32(value.to_bytes())),
        expected_series_revision: request.expected_series_revision(),
        expected_ticket_revision: request.expected_ticket_revision(),
        request_base64: BASE64.encode(request_bytes),
        request_sha256: sha256_hex(request_bytes),
        phase: SeriesTerminalJournalPhaseV1::Planned,
        physical: None,
        payer: None,
        prestate: None,
        poststate: None,
        packet: None,
        finalization: None,
        intent_sha256: String::new(),
        state_sha256: String::new(),
    };
    journal.intent_sha256 = journal_intent_digest_v1(&journal)?;
    refresh_journal_digest_v1(&mut journal)?;
    authenticate_series_terminal_journal_v1(&journal)?;
    Ok(journal)
}

/// Capture a complete same-slot projection. Callers pass the selected physical
/// instruction's unique writable accounts plus the distinct fee payer.
pub(crate) fn build_series_chain_projection_v1(
    ledger: &SeriesLedgerIdentityV1,
    finalized_slot: u64,
    accounts: Vec<SeriesObservedAccountSlotV1>,
) -> Result<SeriesChainProjectionV1> {
    authenticate_ledger_identity_v1(ledger)?;
    if finalized_slot == 0 || accounts.is_empty() {
        return Err(refusal("Series projection was empty or not finalized"));
    }
    let mut durable = BTreeMap::new();
    for slot in accounts {
        let key = slot.key.to_string();
        let value = match slot.account {
            Some(account) => {
                if account.key != slot.key {
                    return Err(refusal("Series account observation changed its key"));
                }
                durable_present_account_v1(account)?
            }
            None => durable_absent_account_v1(slot.key),
        };
        if durable.insert(key, value).is_some() {
            return Err(refusal("Series projection repeated an account key"));
        }
    }
    let mut projection = SeriesChainProjectionV1 {
        ledger_identity_sha256: ledger.identity_sha256.clone(),
        finalized_slot,
        accounts: durable,
        state_sha256: String::new(),
    };
    projection.state_sha256 = projection_digest_v1(&projection)?;
    authenticate_projection_v1(&projection)?;
    Ok(projection)
}

/// Add the selected generic-Hot frame and exact writable prestate.
pub(crate) fn prepare_series_terminal_journal_v1<S: SelectedSeriesPhysicalActionV1>(
    current: &SeriesTerminalJournalV1,
    selected: &S,
    prestate: SeriesChainProjectionV1,
    payer: Pubkey,
) -> Result<SeriesTerminalJournalV1> {
    authenticate_series_terminal_journal_v1(current)?;
    authenticate_projection_v1(&prestate)?;
    if current.phase != SeriesTerminalJournalPhaseV1::Planned
        || prestate.ledger_identity_sha256 != current.ledger.identity_sha256
        || prestate.finalized_slot != current.planner_finalized_slot
    {
        return Err(refusal(
            "Series prepare changed the planned ledger or finalized slot",
        ));
    }
    let physical = durable_physical_from_selected_v1(current, selected)?;
    let instruction = physical.instruction()?;
    let roles = selected.role_keys();
    if selected.observation().finality != Finality::Finalized
        || selected.observation().slot != prestate.finalized_slot
    {
        return Err(refusal(
            "selected Series report did not share the Prepared finalized observation",
        ));
    }
    if payer == roles.root || roles.ticket == Some(payer) || roles.rent_credit == Some(payer) {
        return Err(refusal(
            "Series protocol state must not alias the transaction fee payer",
        ));
    }
    if payer == roles.parent_market
        || roles.occurrence_market == Some(payer)
        || roles.occurrence_permit == Some(payer)
    {
        return Err(refusal(
            "Series Market/permit evidence must not alias the transaction fee payer",
        ));
    }
    let writable = instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect::<BTreeSet<_>>();
    let mut projected = writable.clone();
    projected.insert(roles.parent_market);
    if let Some(market) = roles.occurrence_market {
        projected.insert(market);
    }
    if let Some(permit) = roles.occurrence_permit {
        projected.insert(permit);
    }
    if writable.is_empty()
        || !writable.contains(&roles.root)
        || current.action.terminal()
            && !roles
                .rent_credit
                .is_some_and(|credit| writable.contains(&credit))
        || current.action == SeriesJournalActionV1::Retire
            && !roles
                .ticket
                .is_some_and(|ticket| writable.contains(&ticket))
    {
        return Err(refusal(
            "selected generic-Hot frame omitted an authenticated writable terminal role",
        ));
    }
    for key in projected.iter().chain(std::iter::once(&payer)) {
        if !prestate.accounts.contains_key(&key.to_string()) {
            return Err(refusal(
                "Series prestate omitted a writable/Market protocol account or fee payer",
            ));
        }
    }
    if prestate.accounts.len() != projected.len().saturating_add(1) {
        return Err(refusal(
            "Series prestate contained accounts outside the writable frame and payer",
        ));
    }
    let mut next = current.clone();
    next.phase = SeriesTerminalJournalPhaseV1::Prepared;
    next.physical = Some(physical);
    next.payer = Some(payer.to_string());
    next.prestate = Some(prestate);
    refresh_journal_digest_v1(&mut next)?;
    authenticate_transition_v1(current, &next)?;
    Ok(next)
}

/// Reauthenticate the current selected release before signing, polling, or
/// resending a durable packet. The journal is evidence, not authority: its
/// copied instruction cannot become executable unless the selected V5 API
/// independently reproduces every byte, role, and artifact identity.
pub(crate) fn reauthenticate_series_selected_action_v1<S: SelectedSeriesPhysicalActionV1>(
    journal: &SeriesTerminalJournalV1,
    selected: &S,
) -> Result<Instruction> {
    authenticate_series_terminal_journal_v1(journal)?;
    if journal.phase == SeriesTerminalJournalPhaseV1::Planned {
        return Err(refusal(
            "Planned Series journal has no physical action to reauthenticate",
        ));
    }
    let expected = durable_physical_from_selected_v1(journal, selected)?;
    let durable = journal
        .physical
        .as_ref()
        .ok_or_else(|| refusal("Series journal omitted selected physical action"))?;
    if durable != &expected {
        return Err(refusal(
            "current selected Series release no longer reproduced the durable physical action",
        ));
    }
    expected.instruction()
}

fn durable_physical_from_selected_v1<S: SelectedSeriesPhysicalActionV1>(
    journal: &SeriesTerminalJournalV1,
    selected: &S,
) -> Result<DurableSeriesPhysicalActionV1> {
    let request_bytes = decode_base64(&journal.request_base64, "Series planner request")?;
    if selected.canonical_request_bytes() != request_bytes
        || selected.action() != kernel_action(journal.action)
        || selected.observation().finality != Finality::Finalized
        || consequence_text(selected.consequence()) != journal.consequence
    {
        return Err(refusal(
            "selected Series frame changed the planner request, observation, or consequence",
        ));
    }
    let instruction = selected.generic_hot_instruction();
    let trading_program = selected.trading_program();
    if instruction.program_id != trading_program || trading_program == Pubkey::default() {
        return Err(refusal(
            "selected frame was not a top-level instruction to current Trading",
        ));
    }
    let roles = selected.role_keys();
    let mechanism = selected.mechanism();
    validate_mechanism_and_roles_v1(journal.action, mechanism, roles)?;
    let physical_keys = instruction
        .accounts
        .iter()
        .map(|meta| meta.pubkey)
        .collect::<BTreeSet<_>>();
    if [
        Some(roles.root),
        Some(roles.parent_market),
        roles.ticket,
        roles.rent_credit,
        roles.occurrence_market,
        roles.occurrence_permit,
    ]
    .into_iter()
    .flatten()
    .any(|key| !physical_keys.contains(&key))
    {
        return Err(refusal(
            "selected Series role was absent from its generic-Hot instruction",
        ));
    }
    let authority = durable_authority_v1(selected.selected_authority_ids())?;
    let mut physical = DurableSeriesPhysicalActionV1 {
        route: "generic-hot-v3-selected-v5-profile-v3".into(),
        mechanism,
        trading_program: trading_program.to_string(),
        accounts: instruction
            .accounts
            .iter()
            .map(|meta| DurableSeriesInstructionAccountV1 {
                address: meta.pubkey.to_string(),
                signer: meta.is_signer,
                writable: meta.is_writable,
            })
            .collect(),
        data_base64: BASE64.encode(&instruction.data),
        data_sha256: sha256_hex(&instruction.data),
        request_sha256: journal.request_sha256.clone(),
        authority,
        root: roles.root.to_string(),
        ticket: roles.ticket.map(|value| value.to_string()),
        rent_credit: roles.rent_credit.map(|value| value.to_string()),
        parent_market: roles.parent_market.to_string(),
        parent_market_generation: roles.parent_market_generation,
        occurrence_market: roles.occurrence_market.map(|value| value.to_string()),
        occurrence_market_generation: roles.occurrence_market_generation,
        occurrence_permit: roles.occurrence_permit.map(|value| value.to_string()),
        physical_sha256: String::new(),
    };
    physical.physical_sha256 = physical_digest_v1(&physical)?;
    authenticate_physical_v1(journal, &physical)?;
    Ok(physical)
}

/// Bind the shared compiler's exact signed v0 output to the selected Series
/// frame. The caller supplies the compiler-resolved key list; authentication
/// below proves that the packet's final compiled instruction is byte-exact to
/// generic Hot before the journal may cross the fsync-before-send boundary.
pub(crate) fn build_series_terminal_packet_binding_v1(
    prepared: &SeriesTerminalJournalV1,
    signed: SignedVersionedPacketV1,
    payer: Pubkey,
    lookup_table: Pubkey,
    lookup_table_sha256: String,
    resolved_account_keys: Vec<Pubkey>,
) -> Result<SeriesTerminalPacketBindingV1> {
    authenticate_series_terminal_journal_v1(prepared)?;
    if prepared.phase != SeriesTerminalJournalPhaseV1::Prepared {
        return Err(refusal("Series packet binding requires durable Prepared"));
    }
    let mut packet = SeriesTerminalPacketBindingV1 {
        signed,
        payer: payer.to_string(),
        lookup_table: lookup_table.to_string(),
        lookup_table_sha256,
        resolved_account_keys: resolved_account_keys
            .iter()
            .map(ToString::to_string)
            .collect(),
        resolved_account_keys_sha256: sha256_hex(&pubkey_bytes(&resolved_account_keys)),
        packet_binding_sha256: String::new(),
    };
    packet.packet_binding_sha256 = packet_binding_digest_v1(&packet)?;
    authenticate_packet_binding_v1(prepared, &packet)?;
    Ok(packet)
}

/// Persist exact signed bytes before their first submission.
pub(crate) fn dispatch_series_terminal_journal_v1(
    current: &SeriesTerminalJournalV1,
    mut packet: SeriesTerminalPacketBindingV1,
) -> Result<SeriesTerminalJournalV1> {
    authenticate_series_terminal_journal_v1(current)?;
    if current.phase != SeriesTerminalJournalPhaseV1::Prepared {
        return Err(refusal("Series dispatch requires durable Prepared"));
    }
    packet.packet_binding_sha256.clear();
    packet.packet_binding_sha256 = packet_binding_digest_v1(&packet)?;
    authenticate_packet_binding_v1(current, &packet)?;
    let mut next = current.clone();
    next.phase = SeriesTerminalJournalPhaseV1::Dispatching;
    next.packet = Some(packet);
    refresh_journal_digest_v1(&mut next)?;
    authenticate_transition_v1(current, &next)?;
    Ok(next)
}

/// Record that the exact durable packet was submitted. No packet argument is
/// accepted, so this transition cannot substitute bytes after the send.
pub(crate) fn submit_series_terminal_journal_v1(
    current: &SeriesTerminalJournalV1,
    observed_signature: &str,
) -> Result<SeriesTerminalJournalV1> {
    authenticate_series_terminal_journal_v1(current)?;
    let packet = current
        .packet
        .as_ref()
        .ok_or_else(|| refusal("Dispatching Series journal omitted its packet"))?;
    if current.phase != SeriesTerminalJournalPhaseV1::Dispatching
        || observed_signature != packet.signed.signature
    {
        return Err(refusal(
            "Series submission did not return the exact durable signature",
        ));
    }
    let mut next = current.clone();
    next.phase = SeriesTerminalJournalPhaseV1::Submitted;
    refresh_journal_digest_v1(&mut next)?;
    authenticate_transition_v1(current, &next)?;
    Ok(next)
}

/// Bind one successful finalized packet to its exact poststate. Terminal acts
/// additionally return their donation-inclusive conservation receipt.
pub(crate) fn finalize_series_terminal_journal_v1(
    current: &SeriesTerminalJournalV1,
    signature: String,
    packet_sha256: String,
    fee_lamports: u64,
    compute_units_consumed: u64,
    poststate: SeriesChainProjectionV1,
) -> Result<(
    SeriesTerminalJournalV1,
    Option<SeriesTerminalConservationReceiptV1>,
)> {
    authenticate_series_terminal_journal_v1(current)?;
    authenticate_projection_v1(&poststate)?;
    let packet = current
        .packet
        .as_ref()
        .ok_or_else(|| refusal("Submitted Series journal omitted its packet"))?;
    if current.phase != SeriesTerminalJournalPhaseV1::Submitted
        || signature != packet.signed.signature
        || packet_sha256 != packet.signed.packet_sha256
        || poststate.ledger_identity_sha256 != current.ledger.identity_sha256
        || poststate.finalized_slot < current.planner_finalized_slot
    {
        return Err(refusal(
            "Series finalization changed packet, ledger, or observation order",
        ));
    }
    let conservation = if current.action.terminal() {
        Some(build_terminal_conservation_receipt_v1(
            current,
            &poststate,
            fee_lamports,
        )?)
    } else {
        None
    };
    let complete_source_credit_lamports = conservation
        .as_ref()
        .map(|receipt| receipt.source_lamports_before);
    let mut finalization = SeriesTerminalFinalizationV1 {
        signature,
        finalized_slot: poststate.finalized_slot,
        packet_sha256,
        fee_lamports,
        compute_units_consumed,
        poststate_sha256: poststate.state_sha256.clone(),
        complete_source_credit_lamports,
        finalization_sha256: String::new(),
    };
    finalization.finalization_sha256 = finalization_digest_v1(&finalization)?;
    let mut next = current.clone();
    next.phase = SeriesTerminalJournalPhaseV1::Finalized;
    next.poststate = Some(poststate.clone());
    next.finalization = Some(finalization);
    refresh_journal_digest_v1(&mut next)?;
    authenticate_transition_v1(current, &next)?;
    Ok((next, conservation))
}

/// Prove a finalized hostile named the intended refusal and rolled every
/// protocol account back exactly. This does not advance the success journal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_series_terminal_rollback_v1(
    prepared: &SeriesTerminalJournalV1,
    signature: String,
    packet_sha256: String,
    exact_expected_custom_refusal_code: u32,
    exact_observed_custom_refusal_code: u32,
    fee_lamports: u64,
    compute_units_consumed: u64,
    poststate: SeriesChainProjectionV1,
) -> Result<SeriesTerminalRollbackReceiptV1> {
    authenticate_series_terminal_journal_v1(prepared)?;
    authenticate_projection_v1(&poststate)?;
    if !matches!(
        prepared.phase,
        SeriesTerminalJournalPhaseV1::Prepared
            | SeriesTerminalJournalPhaseV1::Dispatching
            | SeriesTerminalJournalPhaseV1::Submitted
    ) || exact_expected_custom_refusal_code == 0
        || exact_observed_custom_refusal_code != exact_expected_custom_refusal_code
        || signature.parse::<Signature>().is_err()
        || poststate.ledger_identity_sha256 != prepared.ledger.identity_sha256
        || poststate.finalized_slot < prepared.planner_finalized_slot
    {
        return Err(refusal(
            "Series hostile lacked the exact finalized custom refusal binding",
        ));
    }
    require_sha256(&packet_sha256, "hostile packet")?;
    let prestate = prepared
        .prestate
        .as_ref()
        .ok_or_else(|| refusal("prepared Series hostile omitted prestate"))?;
    let physical = prepared
        .physical
        .as_ref()
        .ok_or_else(|| refusal("prepared Series hostile omitted physical action"))?;
    let payer = prepared_payer_v1(prepared)?;
    let writable = physical
        .accounts
        .iter()
        .filter(|meta| meta.writable)
        .map(|meta| meta.address.as_str())
        .collect::<BTreeSet<_>>();
    if prestate.accounts.keys().ne(poststate.accounts.keys()) {
        return Err(refusal("Series hostile changed its observed account set"));
    }
    for key in &writable {
        if prestate.accounts.get(*key) != poststate.accounts.get(*key) {
            return Err(refusal(
                "Series hostile did not roll protocol bytes and lamports back exactly",
            ));
        }
    }
    require_payer_fee_only_v1(prestate, &poststate, &payer, fee_lamports)?;
    let mut receipt = SeriesTerminalRollbackReceiptV1 {
        schema: SERIES_TERMINAL_ROLLBACK_SCHEMA_V1.into(),
        campaign_sha256: prepared.campaign_sha256.clone(),
        ledger_identity_sha256: prepared.ledger.identity_sha256.clone(),
        action: prepared.action,
        signature,
        finalized_slot: poststate.finalized_slot,
        packet_sha256,
        exact_custom_refusal_code: exact_observed_custom_refusal_code,
        fee_lamports,
        compute_units_consumed,
        protocol_accounts_byte_and_lamport_exact: true,
        distinct_payer_fee_only: true,
        prestate_sha256: prestate.state_sha256.clone(),
        poststate_sha256: poststate.state_sha256,
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = rollback_receipt_digest_v1(&receipt)?;
    authenticate_series_terminal_rollback_receipt_v1(&receipt)?;
    Ok(receipt)
}

/// Current crash-recovery instruction. Submitted is deliberately poll-only.
pub(crate) fn series_terminal_recovery_v1(
    journal: &SeriesTerminalJournalV1,
) -> Result<SeriesTerminalRecoveryV1> {
    authenticate_series_terminal_journal_v1(journal)?;
    Ok(match journal.phase {
        SeriesTerminalJournalPhaseV1::Planned => {
            SeriesTerminalRecoveryV1::PrepareSelectedGenericHot
        }
        SeriesTerminalJournalPhaseV1::Prepared => {
            SeriesTerminalRecoveryV1::SignOnceAndPersistDispatching
        }
        SeriesTerminalJournalPhaseV1::Dispatching => {
            SeriesTerminalRecoveryV1::PollThenResendIdentical
        }
        SeriesTerminalJournalPhaseV1::Submitted => SeriesTerminalRecoveryV1::PollOnly,
        SeriesTerminalJournalPhaseV1::Finalized => SeriesTerminalRecoveryV1::Complete,
    })
}

/// Read and reauthenticate one canonical durable Series journal. Alternate
/// JSON spellings and duplicate keys are refused so the bytes on disk retain
/// one exact meaning across restarts.
pub(crate) fn read_series_terminal_journal_file_v1(path: &Path) -> Result<SeriesTerminalJournalV1> {
    let source = read_bounded_series_file_v1(path, "Series terminal journal")?;
    let value = parse_json_without_duplicate_keys_v1(&source)?;
    let journal: SeriesTerminalJournalV1 = serde_json::from_value(value)
        .map_err(|error| Error::new(format!("Series terminal journal JSON: {error}")))?;
    authenticate_series_terminal_journal_v1(&journal)?;
    if source != canonical_series_json_v1(&journal)? {
        return Err(refusal(
            "Series terminal journal was not canonical durable JSON",
        ));
    }
    Ok(journal)
}

/// Create the first durable copy of a Series action. The first durable phase
/// is deliberately `Prepared`: a bare `Planned` file could not be resumed
/// after the validator advances because ordinary finalized RPC cannot recover
/// the complete historical account projection at the planner's exact slot.
/// Planning, selected-release authentication, and the same-slot prestate are
/// therefore admitted in memory before `create_new` + fsync. A crash before
/// this boundary sends nothing and can safely replan; a crash after it has all
/// inputs needed to sign exactly once. A reused sequence path is a refusal.
pub(crate) fn create_series_terminal_journal_file_v1(
    path: &Path,
    journal: &SeriesTerminalJournalV1,
) -> Result<()> {
    authenticate_series_terminal_journal_v1(journal)?;
    if journal.phase != SeriesTerminalJournalPhaseV1::Prepared {
        return Err(refusal(
            "first durable Series journal must be same-slot Prepared",
        ));
    }
    require_absolute_series_output_v1(path)?;
    let bytes = canonical_series_json_v1(journal)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            Error::new(format!(
                "create Series terminal journal {}: {error}",
                path.display()
            ))
        })?;
    file.write_all(&bytes).map_err(|error| {
        Error::new(format!(
            "write Series terminal journal {}: {error}",
            path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        Error::new(format!(
            "fsync Series terminal journal {}: {error}",
            path.display()
        ))
    })?;
    sync_series_parent_v1(path)
}

/// Atomically replace one exact authenticated phase with its successor. The
/// expected canonical bytes must still be present immediately before rename;
/// a changed or noncanonical file never gets normalized silently.
pub(crate) fn replace_series_terminal_journal_file_v1(
    path: &Path,
    expected: &SeriesTerminalJournalV1,
    next: &SeriesTerminalJournalV1,
) -> Result<()> {
    authenticate_series_terminal_journal_v1(expected)?;
    authenticate_series_terminal_journal_v1(next)?;
    authenticate_transition_v1(expected, next)?;
    require_absolute_series_output_v1(path)?;
    let expected_bytes = canonical_series_json_v1(expected)?;
    let current = fs::read(path).map_err(|error| {
        Error::new(format!(
            "read Series terminal journal {}: {error}",
            path.display()
        ))
    })?;
    if current != expected_bytes {
        return Err(refusal(
            "Series terminal journal changed between authentication and transition",
        ));
    }
    let next_bytes = canonical_series_json_v1(next)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::new(format!("Series durable clock: {error}")))?
        .as_nanos();
    let temp = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| {
            Error::new(format!(
                "create Series journal replacement {}: {error}",
                temp.display()
            ))
        })?;
    if let Err(error) = (|| -> std::io::Result<()> {
        file.write_all(&next_bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        Ok(())
    })() {
        let _ = fs::remove_file(&temp);
        return Err(Error::new(format!(
            "replace Series terminal journal {}: {error}",
            path.display()
        )));
    }
    sync_series_parent_v1(path)
}

/// Observe every unique writable account in the selected generic-Hot frame
/// plus its distinct fee payer in one finalized getMultipleAccounts response.
/// Exact absence is preserved for lifecycle-created or lifecycle-closed state.
pub(crate) fn observe_selected_series_projection_from_rpc_v1<S: SelectedSeriesPhysicalActionV1>(
    rpc: &mut Rpc,
    journal: &SeriesTerminalJournalV1,
    selected: &S,
    payer: Pubkey,
    minimum_slot: u64,
) -> Result<SeriesChainProjectionV1> {
    authenticate_series_terminal_journal_v1(journal)?;
    let physical = durable_physical_from_selected_v1(journal, selected)?;
    let instruction = physical.instruction()?;
    let mut keys = instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect::<BTreeSet<_>>();
    let roles = selected.role_keys();
    keys.insert(roles.parent_market);
    if let Some(market) = roles.occurrence_market {
        keys.insert(market);
    }
    if let Some(permit) = roles.occurrence_permit {
        keys.insert(permit);
    }
    if keys.contains(&payer) {
        return Err(refusal(
            "Series fee payer aliased a writable protocol account",
        ));
    }
    keys.insert(payer);
    let keys = keys.into_iter().collect::<Vec<_>>();
    let (slot, accounts) = rpc.finalized_accounts(&keys, minimum_slot)?;
    if accounts.len() != keys.len() {
        return Err(refusal(
            "Series finalized account response changed its cardinality",
        ));
    }
    let observed = keys
        .into_iter()
        .zip(accounts)
        .map(|(key, account)| SeriesObservedAccountSlotV1 {
            key,
            account: account.map(|account| SeriesObservedAccountV1 {
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            }),
        })
        .collect();
    build_series_chain_projection_v1(&journal.ledger, slot, observed)
}

/// Reobserve the exact planner slot, admit the selected action, and make the
/// resulting `Prepared` journal the first durable file. A newer finalized
/// snapshot is not silently substituted for the snapshot the planner
/// inspected. If the slot has advanced the caller replans; it never leaves an
/// unrecoverable bare `Planned` file behind.
pub(crate) fn prepare_series_terminal_from_rpc_v1<S: SelectedSeriesPhysicalActionV1>(
    rpc: &mut Rpc,
    path: &Path,
    planned: &SeriesTerminalJournalV1,
    selected: &S,
    payer: Pubkey,
) -> Result<SeriesTerminalJournalV1> {
    if planned.phase != SeriesTerminalJournalPhaseV1::Planned {
        return Err(refusal("Series RPC prepare requires durable Planned"));
    }
    let prestate = observe_selected_series_projection_from_rpc_v1(
        rpc,
        planned,
        selected,
        payer,
        planned.planner_finalized_slot,
    )?;
    if prestate.finalized_slot != planned.planner_finalized_slot {
        return Err(refusal(
            "Series planner snapshot advanced before physical prepare",
        ));
    }
    let next = prepare_series_terminal_journal_v1(planned, selected, prestate, payer)?;
    create_series_terminal_journal_file_v1(path, &next)?;
    Ok(next)
}

/// Sign the selected generic-Hot instruction exactly once and fsync the signed
/// bytes before any send is reachable. The lookup table must be the exact
/// frozen, activated table admitted by the current release exterior.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_series_terminal_from_rpc_v1<S: SelectedSeriesPhysicalActionV1>(
    rpc: &mut Rpc,
    path: &Path,
    prepared: &SeriesTerminalJournalV1,
    selected: &S,
    payer: &Keypair,
    additional_signers: &[&Keypair],
    lookup_table_key: Pubkey,
    lookup_table_sha256: &str,
    lookup_table: &ObservedAccount,
) -> Result<SeriesTerminalJournalV1> {
    if prepared.phase != SeriesTerminalJournalPhaseV1::Prepared {
        return Err(refusal("Series RPC dispatch requires durable Prepared"));
    }
    let instruction = reauthenticate_series_selected_action_v1(prepared, selected)?;
    if prepared_payer_v1(prepared)? != payer.pubkey().to_string() {
        return Err(refusal(
            "Series signing key did not name the prepared fee payer",
        ));
    }
    authenticate_series_lookup_table_v1(lookup_table, lookup_table_key, lookup_table_sha256)?;
    let signed = rpc.prepare_signed_v0_packet_with_signers(
        series_rpc_label_v1(prepared.action),
        std::slice::from_ref(&instruction),
        payer,
        additional_signers,
        lookup_table,
    )?;
    Rpc::authenticate_signed_v0_packet(
        series_rpc_label_v1(prepared.action),
        std::slice::from_ref(&instruction),
        payer.pubkey(),
        lookup_table,
        &signed,
    )?;
    let resolved = resolve_series_packet_keys_v1(&signed, lookup_table_key, lookup_table)?;
    let binding = build_series_terminal_packet_binding_v1(
        prepared,
        signed,
        payer.pubkey(),
        lookup_table_key,
        lookup_table_sha256.to_owned(),
        resolved,
    )?;
    let next = dispatch_series_terminal_journal_v1(prepared, binding)?;
    replace_series_terminal_journal_file_v1(path, prepared, &next)?;
    Ok(next)
}

/// Execute one recovery pass. Dispatching first polls finalized history and
/// otherwise resends the identical fsynced bytes exactly once. Submitted is
/// strictly poll-only. A successful history entry is bound to a fresh same-
/// ledger finalized poststate and atomically advances the journal to Finalized.
pub(crate) fn advance_series_terminal_from_rpc_v1<S: SelectedSeriesPhysicalActionV1>(
    rpc: &mut Rpc,
    path: &Path,
    current: &SeriesTerminalJournalV1,
    selected: &S,
    lookup_table_key: Pubkey,
    lookup_table_sha256: &str,
    lookup_table: &ObservedAccount,
) -> Result<SeriesTerminalRpcAdvanceV1> {
    let instruction = reauthenticate_series_selected_action_v1(current, selected)?;
    let packet = current
        .packet
        .as_ref()
        .ok_or_else(|| refusal("active Series journal omitted durable packet"))?;
    let signature = Signature::from_str(&packet.signed.signature)
        .map_err(|error| Error::new(format!("Series durable signature: {error}")))?;
    // Poll-only recovery is still an authentication boundary. In particular,
    // Submitted must not trust the journal's weaker resolved-key projection:
    // reproduce the complete canonical bounded message (including its exact
    // ComputeBudget prefix) from the current frozen table before accepting the
    // durable signature as the action we intended to poll.
    authenticate_series_lookup_table_v1(lookup_table, lookup_table_key, lookup_table_sha256)?;
    Rpc::authenticate_signed_v0_packet(
        series_rpc_label_v1(current.action),
        std::slice::from_ref(&instruction),
        parse_pubkey(&packet.payer, "Series packet payer")?,
        lookup_table,
        &packet.signed,
    )?;
    let resolved = resolve_series_packet_keys_v1(&packet.signed, lookup_table_key, lookup_table)?;
    if resolved.iter().map(ToString::to_string).collect::<Vec<_>>() != packet.resolved_account_keys
        || lookup_table_sha256 != packet.lookup_table_sha256
        || lookup_table_key.to_string() != packet.lookup_table
    {
        return Err(refusal(
            "Series recovery routing no longer matched the durable packet binding",
        ));
    }
    match current.phase {
        SeriesTerminalJournalPhaseV1::Dispatching => {
            if let Some(finalized) =
                rpc.finalized_signed_packet(series_rpc_label_v1(current.action), signature, false)?
            {
                let submitted =
                    submit_series_terminal_journal_v1(current, &packet.signed.signature)?;
                replace_series_terminal_journal_file_v1(path, current, &submitted)?;
                return finalize_series_terminal_from_rpc_v1(
                    rpc, path, &submitted, selected, finalized,
                );
            }
            let bytes = decode_series_packet_v1(&packet.signed)?;
            let returned = rpc.submit_signed_packet_once(
                series_rpc_label_v1(current.action),
                &bytes,
                signature,
                false,
            )?;
            let next = submit_series_terminal_journal_v1(current, &returned.to_string())?;
            replace_series_terminal_journal_file_v1(path, current, &next)?;
            Ok(SeriesTerminalRpcAdvanceV1::Pending(next))
        }
        SeriesTerminalJournalPhaseV1::Submitted => {
            let Some(finalized) =
                rpc.finalized_signed_packet(series_rpc_label_v1(current.action), signature, false)?
            else {
                return Ok(SeriesTerminalRpcAdvanceV1::Pending(current.clone()));
            };
            finalize_series_terminal_from_rpc_v1(rpc, path, current, selected, finalized)
        }
        SeriesTerminalJournalPhaseV1::Finalized => {
            let conservation = if current.action.terminal() {
                let poststate = current
                    .poststate
                    .as_ref()
                    .ok_or_else(|| refusal("Finalized Series journal omitted poststate"))?;
                let finalization = current
                    .finalization
                    .as_ref()
                    .ok_or_else(|| refusal("Finalized Series journal omitted finalization"))?;
                Some(build_terminal_conservation_receipt_v1(
                    current,
                    poststate,
                    finalization.fee_lamports,
                )?)
            } else {
                None
            };
            Ok(SeriesTerminalRpcAdvanceV1::Finalized {
                journal: current.clone(),
                conservation,
            })
        }
        SeriesTerminalJournalPhaseV1::Planned | SeriesTerminalJournalPhaseV1::Prepared => Err(
            refusal("Series RPC advance requires Dispatching, Submitted, or Finalized"),
        ),
    }
}

fn finalize_series_terminal_from_rpc_v1<S: SelectedSeriesPhysicalActionV1>(
    rpc: &mut Rpc,
    path: &Path,
    submitted: &SeriesTerminalJournalV1,
    selected: &S,
    finalized: FinalizedSignedPacketV1,
) -> Result<SeriesTerminalRpcAdvanceV1> {
    let packet = submitted
        .packet
        .as_ref()
        .ok_or_else(|| refusal("submitted Series journal omitted packet"))?;
    if sha256_hex(&finalized.packet) != packet.signed.packet_sha256 {
        return Err(refusal(
            "finalized Series transaction bytes differed from the durable packet",
        ));
    }
    let fee = finalized
        .evidence
        .fee_lamports
        .ok_or_else(|| refusal("finalized Series transaction omitted exact fee"))?;
    let compute_units = finalized
        .evidence
        .compute_units_consumed
        .ok_or_else(|| refusal("finalized Series transaction omitted compute units"))?;
    let payer = parse_pubkey(&packet.payer, "Series packet payer")?;
    let poststate = observe_selected_series_projection_from_rpc_v1(
        rpc,
        submitted,
        selected,
        payer,
        finalized.evidence.slot,
    )?;
    let (next, conservation) = finalize_series_terminal_journal_v1(
        submitted,
        finalized.evidence.signature,
        packet.signed.packet_sha256.clone(),
        fee,
        compute_units,
        poststate,
    )?;
    replace_series_terminal_journal_file_v1(path, submitted, &next)?;
    Ok(SeriesTerminalRpcAdvanceV1::Finalized {
        journal: next,
        conservation,
    })
}

fn authenticate_series_lookup_table_v1(
    table: &ObservedAccount,
    expected_key: Pubkey,
    expected_sha256: &str,
) -> Result<()> {
    require_sha256(expected_sha256, "Series lookup table")?;
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| refusal("Series lookup table bytes did not decode"))?;
    if table.key != expected_key
        || table.owner != lookup_table_program::id()
        || table.executable
        || decoded.meta.authority.is_some()
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= table.observation.slot
        || decoded.addresses.is_empty()
        || sha256_hex(&table.data) != expected_sha256
    {
        return Err(refusal(
            "Series lookup table was not the exact frozen activated routing table",
        ));
    }
    Ok(())
}

fn resolve_series_packet_keys_v1(
    signed: &SignedVersionedPacketV1,
    table_key: Pubkey,
    table: &ObservedAccount,
) -> Result<Vec<Pubkey>> {
    let bytes = decode_series_packet_v1(signed)?;
    let transaction: VersionedTransaction = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("Series routed packet: {error}")))?;
    let VersionedMessage::V0(message) = transaction.message else {
        return Err(refusal("Series routed packet was not v0"));
    };
    if message.address_table_lookups.len() != 1
        || message.address_table_lookups[0].account_key != table_key
    {
        return Err(refusal(
            "Series routed packet changed its one exact lookup table",
        ));
    }
    let lookup = &message.address_table_lookups[0];
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| refusal("Series lookup table bytes did not decode"))?;
    let address = |index: u8| -> Result<Pubkey> {
        decoded
            .addresses
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| refusal("Series lookup index exceeded frozen table"))
    };
    let mut result = message.account_keys.clone();
    result.extend(
        lookup
            .writable_indexes
            .iter()
            .copied()
            .map(address)
            .collect::<Result<Vec<_>>>()?,
    );
    result.extend(
        lookup
            .readonly_indexes
            .iter()
            .copied()
            .map(address)
            .collect::<Result<Vec<_>>>()?,
    );
    Ok(result)
}

fn decode_series_packet_v1(packet: &SignedVersionedPacketV1) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(&packet.packet_base64)
        .map_err(|error| Error::new(format!("Series packet base64: {error}")))?;
    if BASE64.encode(&bytes) != packet.packet_base64 || sha256_hex(&bytes) != packet.packet_sha256 {
        return Err(refusal("Series packet bytes or digest changed"));
    }
    Ok(bytes)
}

const fn series_rpc_label_v1(action: SeriesJournalActionV1) -> &'static str {
    match action {
        SeriesJournalActionV1::Prepare => "series-prepare-generic-hot-v5",
        SeriesJournalActionV1::Consume => "series-consume-generic-hot-v5",
        SeriesJournalActionV1::Expire => "series-expire-generic-hot-v5",
        SeriesJournalActionV1::Retire => "series-retire-generic-hot-v5",
        SeriesJournalActionV1::Close => "series-close-generic-hot-v5",
    }
}

fn read_bounded_series_file_v1(path: &Path, label: &str) -> Result<Vec<u8>> {
    require_absolute_series_output_v1(path)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("read {label} {}: {error}", path.display())))?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > 16 * 1024 * 1024 {
        return Err(refusal(format!(
            "{label} was not a regular 1..16777216 byte file"
        )));
    }
    fs::read(path).map_err(|error| Error::new(format!("read {label} {}: {error}", path.display())))
}

fn canonical_series_json_v1<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| Error::new(format!("canonical Series JSON: {error}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn require_absolute_series_output_v1(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(refusal("Series durable output path was not absolute"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| refusal("Series durable output omitted a parent directory"))?;
    let metadata = fs::symlink_metadata(parent).map_err(|error| {
        Error::new(format!(
            "Series durable output parent {}: {error}",
            parent.display()
        ))
    })?;
    if !metadata.file_type().is_dir() {
        return Err(refusal(
            "Series durable output parent was not a real directory",
        ));
    }
    Ok(())
}

fn sync_series_parent_v1(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| refusal("Series durable output omitted a parent directory"))?;
    OpenOptions::new()
        .read(true)
        .open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            Error::new(format!(
                "fsync Series durable output parent {}: {error}",
                parent.display()
            ))
        })
}

pub(crate) fn authenticate_series_terminal_journal_v1(
    journal: &SeriesTerminalJournalV1,
) -> Result<()> {
    if journal.schema != SERIES_TERMINAL_JOURNAL_SCHEMA_V1
        || journal.sequence == u32::MAX
        || journal.planner_finalized_slot == 0
        || journal.consequence
            != consequence_text(consequence_for_action(kernel_action(journal.action)))
        || journal.request_sha256
            != sha256_hex(&decode_base64(&journal.request_base64, "Series request")?)
        || journal.intent_sha256 != journal_intent_digest_v1(journal)?
        || journal.state_sha256 != journal_state_digest_v1(journal)?
    {
        return Err(refusal(
            "Series journal identity, request, or digest changed",
        ));
    }
    authenticate_ledger_identity_v1(&journal.ledger)?;
    require_sha256(&journal.campaign_sha256, "Series campaign")?;
    require_sha256(&journal.planner_snapshot_sha256, "Series planner snapshot")?;
    let request_bytes = decode_base64(&journal.request_base64, "Series request")?;
    let request = SeriesActionRequestV3::decode(&request_bytes)
        .map_err(|_| refusal("durable Series request no longer decoded"))?;
    if request.action() != kernel_action(journal.action)
        || hex32(request.template().to_bytes()) != journal.template
        || request.occurrence().map(|value| hex32(value.to_bytes())) != journal.occurrence
        || request.ticket().map(|value| hex32(value.to_bytes())) != journal.ticket
        || request.expected_series_revision() != journal.expected_series_revision
        || request.expected_ticket_revision() != journal.expected_ticket_revision
    {
        return Err(refusal("durable Series request facts changed"));
    }
    let phase_shape = match journal.phase {
        SeriesTerminalJournalPhaseV1::Planned => {
            journal.physical.is_none()
                && journal.payer.is_none()
                && journal.prestate.is_none()
                && journal.poststate.is_none()
                && journal.packet.is_none()
                && journal.finalization.is_none()
        }
        SeriesTerminalJournalPhaseV1::Prepared => {
            journal.physical.is_some()
                && journal.payer.is_some()
                && journal.prestate.is_some()
                && journal.poststate.is_none()
                && journal.packet.is_none()
                && journal.finalization.is_none()
        }
        SeriesTerminalJournalPhaseV1::Dispatching | SeriesTerminalJournalPhaseV1::Submitted => {
            journal.physical.is_some()
                && journal.payer.is_some()
                && journal.prestate.is_some()
                && journal.poststate.is_none()
                && journal.packet.is_some()
                && journal.finalization.is_none()
        }
        SeriesTerminalJournalPhaseV1::Finalized => {
            journal.physical.is_some()
                && journal.payer.is_some()
                && journal.prestate.is_some()
                && journal.poststate.is_some()
                && journal.packet.is_some()
                && journal.finalization.is_some()
        }
    };
    if !phase_shape {
        return Err(refusal(
            "Series journal phase omitted or invented durable state",
        ));
    }
    if let Some(physical) = journal.physical.as_ref() {
        authenticate_physical_v1(journal, physical)?;
    }
    if let Some(payer) = journal.payer.as_ref() {
        parse_pubkey(payer, "Series fee payer")?;
    }
    if let Some(prestate) = journal.prestate.as_ref() {
        authenticate_projection_v1(prestate)?;
        if prestate.ledger_identity_sha256 != journal.ledger.identity_sha256
            || prestate.finalized_slot != journal.planner_finalized_slot
        {
            return Err(refusal(
                "Series durable prestate changed its planned observation",
            ));
        }
    }
    if let Some(poststate) = journal.poststate.as_ref() {
        authenticate_projection_v1(poststate)?;
        if poststate.ledger_identity_sha256 != journal.ledger.identity_sha256 {
            return Err(refusal("Series durable poststate changed its ledger"));
        }
    }
    if let Some(packet) = journal.packet.as_ref() {
        authenticate_packet_binding_v1(journal, packet)?;
    }
    if let Some(finalization) = journal.finalization.as_ref() {
        authenticate_finalization_v1(journal, finalization)?;
    }
    Ok(())
}

pub(crate) fn authenticate_series_terminal_rollback_receipt_v1(
    receipt: &SeriesTerminalRollbackReceiptV1,
) -> Result<()> {
    if receipt.schema != SERIES_TERMINAL_ROLLBACK_SCHEMA_V1
        || receipt.exact_custom_refusal_code == 0
        || !receipt.protocol_accounts_byte_and_lamport_exact
        || !receipt.distinct_payer_fee_only
        || receipt.finalized_slot == 0
        || receipt.signature.parse::<Signature>().is_err()
        || receipt.receipt_sha256 != rollback_receipt_digest_v1(receipt)?
    {
        return Err(refusal(
            "Series rollback receipt changed or made a weak claim",
        ));
    }
    for digest in [
        &receipt.campaign_sha256,
        &receipt.ledger_identity_sha256,
        &receipt.packet_sha256,
        &receipt.prestate_sha256,
        &receipt.poststate_sha256,
    ] {
        require_sha256(digest, "Series rollback digest")?;
    }
    Ok(())
}

fn build_terminal_conservation_receipt_v1(
    journal: &SeriesTerminalJournalV1,
    poststate: &SeriesChainProjectionV1,
    fee_lamports: u64,
) -> Result<SeriesTerminalConservationReceiptV1> {
    let prestate = journal
        .prestate
        .as_ref()
        .ok_or_else(|| refusal("terminal conservation omitted prestate"))?;
    let physical = journal
        .physical
        .as_ref()
        .ok_or_else(|| refusal("terminal conservation omitted physical action"))?;
    if prestate.accounts.keys().ne(poststate.accounts.keys()) {
        return Err(refusal(
            "terminal poststate changed its observed account set",
        ));
    }
    let source = match journal.action {
        SeriesJournalActionV1::Retire => physical
            .ticket
            .as_ref()
            .ok_or_else(|| refusal("Retire omitted its Ticket role"))?,
        SeriesJournalActionV1::Close => &physical.root,
        _ => {
            return Err(refusal(
                "nonterminal Series action requested terminal conservation",
            ));
        }
    };
    let credit = physical
        .rent_credit
        .as_ref()
        .ok_or_else(|| refusal("terminal Series action omitted RentCredit"))?;
    let source_before = require_present(prestate, source, "terminal source")?;
    let source_after = require_account(poststate, source, "terminal source")?;
    if source_after.present {
        return Err(refusal("terminal Series source was not deleted"));
    }
    let credit_before = require_present(prestate, credit, "lifecycle RentCredit")?;
    let credit_after = require_present(poststate, credit, "lifecycle RentCredit")?;
    require_same_account_except_lamports(credit_before, credit_after, "lifecycle RentCredit")?;
    let source_lamports = source_before
        .lamports
        .ok_or_else(|| refusal("terminal source omitted lamports"))?;
    let credit_before_lamports = credit_before
        .lamports
        .ok_or_else(|| refusal("RentCredit prestate omitted lamports"))?;
    let expected_credit_after = credit_before_lamports
        .checked_add(source_lamports)
        .ok_or_else(|| refusal("terminal donation-inclusive credit overflowed"))?;
    if credit_after.lamports != Some(expected_credit_after) {
        return Err(refusal(
            "terminal RentCredit did not receive the source's complete observed balance",
        ));
    }
    validate_other_terminal_accounts_v1(journal, poststate, source, credit)?;
    let payer = prepared_payer_v1(journal)?;
    require_payer_fee_only_v1(prestate, poststate, &payer, fee_lamports)?;
    let mut receipt = SeriesTerminalConservationReceiptV1 {
        schema: SERIES_TERMINAL_CONSERVATION_SCHEMA_V1.into(),
        campaign_sha256: journal.campaign_sha256.clone(),
        ledger_identity_sha256: journal.ledger.identity_sha256.clone(),
        action: journal.action,
        source: source.clone(),
        rent_credit: credit.clone(),
        source_lamports_before: source_lamports,
        rent_credit_lamports_before: credit_before_lamports,
        rent_credit_lamports_after: expected_credit_after,
        donation_inclusive_exact_credit: true,
        payer,
        fee_lamports,
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = conservation_receipt_digest_v1(&receipt)?;
    authenticate_conservation_receipt_v1(&receipt)?;
    Ok(receipt)
}

fn validate_other_terminal_accounts_v1(
    journal: &SeriesTerminalJournalV1,
    poststate: &SeriesChainProjectionV1,
    source: &str,
    credit: &str,
) -> Result<()> {
    let prestate = journal.prestate.as_ref().expect("authenticated prestate");
    let physical = journal.physical.as_ref().expect("authenticated physical");
    let payer = prepared_payer_v1(journal)?;
    for (key, before) in &prestate.accounts {
        if key == source || key == credit || key == &payer {
            continue;
        }
        let after = poststate
            .accounts
            .get(key)
            .ok_or_else(|| refusal("terminal poststate omitted an observed account"))?;
        if journal.action == SeriesJournalActionV1::Retire && key == &physical.root {
            require_same_account_except_data(before, after, "Series root")?;
        } else if before != after {
            return Err(refusal(
                "terminal action changed an account outside its selected source/root/RentCredit",
            ));
        }
    }
    Ok(())
}

fn require_payer_fee_only_v1(
    before: &SeriesChainProjectionV1,
    after: &SeriesChainProjectionV1,
    payer: &str,
    fee_lamports: u64,
) -> Result<()> {
    let before = require_present(before, payer, "fee payer")?;
    let after = require_present(after, payer, "fee payer")?;
    require_same_account_except_lamports(before, after, "fee payer")?;
    let expected = before
        .lamports
        .and_then(|value| value.checked_sub(fee_lamports))
        .ok_or_else(|| refusal("fee payer balance underflowed"))?;
    if after.lamports != Some(expected) {
        return Err(refusal(
            "fee payer changed beyond the exact transaction fee",
        ));
    }
    Ok(())
}

fn authenticate_conservation_receipt_v1(
    receipt: &SeriesTerminalConservationReceiptV1,
) -> Result<()> {
    if receipt.schema != SERIES_TERMINAL_CONSERVATION_SCHEMA_V1
        || !receipt.action.terminal()
        || !receipt.donation_inclusive_exact_credit
        || receipt.rent_credit_lamports_after
            != receipt
                .rent_credit_lamports_before
                .checked_add(receipt.source_lamports_before)
                .ok_or_else(|| refusal("conservation receipt overflowed"))?
        || receipt.receipt_sha256 != conservation_receipt_digest_v1(receipt)?
    {
        return Err(refusal("Series terminal conservation receipt changed"));
    }
    for digest in [&receipt.campaign_sha256, &receipt.ledger_identity_sha256] {
        require_sha256(digest, "Series conservation digest")?;
    }
    parse_pubkey(&receipt.source, "Series terminal source")?;
    parse_pubkey(&receipt.rent_credit, "Series terminal RentCredit")?;
    parse_pubkey(&receipt.payer, "Series terminal payer")?;
    Ok(())
}

fn authenticate_transition_v1(
    previous: &SeriesTerminalJournalV1,
    next: &SeriesTerminalJournalV1,
) -> Result<()> {
    authenticate_series_terminal_journal_v1(previous)?;
    authenticate_series_terminal_journal_v1(next)?;
    let legal = matches!(
        (previous.phase, next.phase),
        (
            SeriesTerminalJournalPhaseV1::Planned,
            SeriesTerminalJournalPhaseV1::Prepared
        ) | (
            SeriesTerminalJournalPhaseV1::Prepared,
            SeriesTerminalJournalPhaseV1::Dispatching
        ) | (
            SeriesTerminalJournalPhaseV1::Dispatching,
            SeriesTerminalJournalPhaseV1::Submitted
        ) | (
            SeriesTerminalJournalPhaseV1::Submitted,
            SeriesTerminalJournalPhaseV1::Finalized
        )
    );
    if !legal
        || previous.intent_sha256 != next.intent_sha256
        || previous.campaign_sha256 != next.campaign_sha256
        || previous.sequence != next.sequence
        || previous.request_sha256 != next.request_sha256
        || (previous.physical.is_some() && previous.physical != next.physical)
        || (previous.payer.is_some() && previous.payer != next.payer)
        || (previous.prestate.is_some() && previous.prestate != next.prestate)
        || (previous.poststate.is_some() && previous.poststate != next.poststate)
        || (previous.packet.is_some() && previous.packet != next.packet)
        || (previous.finalization.is_some() && previous.finalization != next.finalization)
    {
        return Err(refusal(
            "Series journal transition skipped, reversed, or changed durable intent",
        ));
    }
    Ok(())
}

fn authenticate_physical_v1(
    journal: &SeriesTerminalJournalV1,
    physical: &DurableSeriesPhysicalActionV1,
) -> Result<()> {
    if physical.route != "generic-hot-v3-selected-v5-profile-v3"
        || physical.request_sha256 != journal.request_sha256
        || physical.physical_sha256 != physical_digest_v1(physical)?
        || physical.data_sha256
            != sha256_hex(&decode_base64(
                &physical.data_base64,
                "Series Hot instruction",
            )?)
        || physical.accounts.is_empty()
    {
        return Err(refusal(
            "Series physical action was not the selected generic-Hot frame",
        ));
    }
    physical.instruction()?;
    parse_pubkey(&physical.root, "Series root")?;
    if let Some(ticket) = physical.ticket.as_ref() {
        parse_pubkey(ticket, "Series Ticket")?;
    }
    if let Some(credit) = physical.rent_credit.as_ref() {
        parse_pubkey(credit, "Series RentCredit")?;
    }
    parse_pubkey(&physical.parent_market, "Series parent Market")?;
    if let Some(market) = physical.occurrence_market.as_ref() {
        parse_pubkey(market, "Series occurrence Market")?;
    }
    if let Some(permit) = physical.occurrence_permit.as_ref() {
        parse_pubkey(permit, "Series occurrence permit")?;
    }
    validate_mechanism_and_roles_v1(
        journal.action,
        physical.mechanism,
        SeriesPhysicalRoleKeysV1 {
            root: parse_pubkey(&physical.root, "Series root")?,
            ticket: physical
                .ticket
                .as_ref()
                .map(|value| parse_pubkey(value, "Series Ticket"))
                .transpose()?,
            rent_credit: physical
                .rent_credit
                .as_ref()
                .map(|value| parse_pubkey(value, "Series RentCredit"))
                .transpose()?,
            parent_market: parse_pubkey(&physical.parent_market, "Series parent Market")?,
            parent_market_generation: physical.parent_market_generation,
            occurrence_market: physical
                .occurrence_market
                .as_ref()
                .map(|value| parse_pubkey(value, "Series occurrence Market"))
                .transpose()?,
            occurrence_market_generation: physical.occurrence_market_generation,
            occurrence_permit: physical
                .occurrence_permit
                .as_ref()
                .map(|value| parse_pubkey(value, "Series occurrence permit"))
                .transpose()?,
        },
    )?;
    authenticate_durable_authority_v1(&physical.authority)?;
    Ok(())
}

fn validate_mechanism_and_roles_v1(
    action: SeriesJournalActionV1,
    mechanism: SeriesPhysicalMechanismV1,
    roles: SeriesPhysicalRoleKeysV1,
) -> Result<()> {
    let role_keys = [
        Some(roles.root),
        Some(roles.parent_market),
        roles.ticket,
        roles.rent_credit,
        roles.occurrence_market,
        roles.occurrence_permit,
    ];
    let present_count = role_keys.iter().flatten().count();
    let distinct = role_keys.into_iter().flatten().collect::<BTreeSet<_>>();
    if distinct.contains(&Pubkey::default())
        || distinct.len() != present_count
        || roles.parent_market_generation == 0
    {
        return Err(refusal(
            "selected Series physical roles aliased or were zero",
        ));
    }
    let valid = match action {
        SeriesJournalActionV1::Prepare | SeriesJournalActionV1::Consume => {
            mechanism == SeriesPhysicalMechanismV1::Occurrence
                && roles.ticket.is_some()
                && roles.occurrence_market.is_some()
                && roles
                    .occurrence_market_generation
                    .is_some_and(|value| value > 0)
                && roles.occurrence_permit.is_none()
        }
        SeriesJournalActionV1::Expire => {
            mechanism == SeriesPhysicalMechanismV1::Occurrence
                && roles.ticket.is_some()
                && roles.occurrence_market.is_some()
                && roles
                    .occurrence_market_generation
                    .is_some_and(|value| value > 0)
                && roles.occurrence_permit.is_some()
        }
        SeriesJournalActionV1::Retire => {
            mechanism == SeriesPhysicalMechanismV1::RetireFundingV5Close
                && roles.ticket.is_some()
                && roles.rent_credit.is_some()
                && roles.occurrence_market.is_none()
                && roles.occurrence_market_generation.is_none()
                && roles.occurrence_permit.is_none()
        }
        SeriesJournalActionV1::Close => {
            mechanism == SeriesPhysicalMechanismV1::CloseLifecycleOnlyRoot
                && roles.ticket.is_none()
                && roles.rent_credit.is_some()
                && roles.occurrence_market.is_none()
                && roles.occurrence_market_generation.is_none()
                && roles.occurrence_permit.is_none()
        }
    };
    if !valid {
        return Err(refusal(
            "selected Series action did not prove its exact V5 terminal mechanism",
        ));
    }
    Ok(())
}

fn durable_authority_v1(
    ids: SeriesSelectedAuthorityIdsV1,
) -> Result<DurableSeriesSelectedAuthorityV1> {
    if [
        ids.release_set,
        ids.program_set_v2,
        ids.descriptor,
        ids.profile_v3,
        ids.request_profile,
        ids.lifecycle_policy,
        ids.strategy,
        ids.transition,
        ids.effect_v5,
    ]
    .contains(&[0; 32])
    {
        return Err(refusal(
            "selected Series authority contained a zero identity",
        ));
    }
    let mut value = DurableSeriesSelectedAuthorityV1 {
        release_set: hex32(ids.release_set),
        program_set_v2: hex32(ids.program_set_v2),
        descriptor: hex32(ids.descriptor),
        profile_v3: hex32(ids.profile_v3),
        request_profile: hex32(ids.request_profile),
        lifecycle_policy: hex32(ids.lifecycle_policy),
        strategy: hex32(ids.strategy),
        transition: hex32(ids.transition),
        effect_v5: hex32(ids.effect_v5),
        authority_sha256: String::new(),
    };
    value.authority_sha256 = authority_digest_v1(&value)?;
    Ok(value)
}

fn authenticate_durable_authority_v1(authority: &DurableSeriesSelectedAuthorityV1) -> Result<()> {
    for value in [
        &authority.release_set,
        &authority.program_set_v2,
        &authority.descriptor,
        &authority.profile_v3,
        &authority.request_profile,
        &authority.lifecycle_policy,
        &authority.strategy,
        &authority.transition,
        &authority.effect_v5,
    ] {
        require_sha256(value, "selected Series artifact identity")?;
        if value.bytes().all(|byte| byte == b'0') {
            return Err(refusal("selected Series artifact identity was zero"));
        }
    }
    if authority.authority_sha256 != authority_digest_v1(authority)? {
        return Err(refusal("selected Series authority digest changed"));
    }
    Ok(())
}

fn authenticate_packet_binding_v1(
    journal: &SeriesTerminalJournalV1,
    packet: &SeriesTerminalPacketBindingV1,
) -> Result<()> {
    if packet.packet_binding_sha256 != packet_binding_digest_v1(packet)? {
        return Err(refusal("Series packet binding digest changed"));
    }
    require_sha256(&packet.lookup_table_sha256, "Series lookup table")?;
    require_sha256(
        &packet.resolved_account_keys_sha256,
        "Series resolved account keys",
    )?;
    if journal.payer.as_ref() != Some(&packet.payer) {
        return Err(refusal("Series packet changed the durable fee payer"));
    }
    let payer = parse_pubkey(&packet.payer, "Series payer")?;
    parse_pubkey(&packet.lookup_table, "Series lookup table")?;
    let resolved = packet
        .resolved_account_keys
        .iter()
        .map(|key| parse_pubkey(key, "Series resolved key"))
        .collect::<Result<Vec<_>>>()?;
    if resolved.is_empty()
        || sha256_hex(&pubkey_bytes(&resolved)) != packet.resolved_account_keys_sha256
    {
        return Err(refusal("Series resolved key projection changed"));
    }
    let physical = journal
        .physical
        .as_ref()
        .ok_or_else(|| refusal("Series packet omitted physical action"))?;
    let bytes = decode_base64(&packet.signed.packet_base64, "Series signed packet")?;
    if sha256_hex(&bytes) != packet.signed.packet_sha256 {
        return Err(refusal("Series signed packet digest changed"));
    }
    let transaction: VersionedTransaction = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("Series signed transaction: {error}")))?;
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("Series signed packet signature: {error}")))?;
    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| refusal("Series transaction was unsigned"))?;
    if signature.to_string() != packet.signed.signature
        || transaction.message.static_account_keys().first() != Some(&payer)
    {
        return Err(refusal("Series signed packet changed signature or payer"));
    }
    let message = match &transaction.message {
        solana_sdk::message::VersionedMessage::V0(message) => message,
        solana_sdk::message::VersionedMessage::Legacy(_) => {
            return Err(refusal("Series physical action was not a v0 packet"));
        }
    };
    let compiled = message
        .instructions
        .last()
        .ok_or_else(|| refusal("Series packet omitted generic Hot"))?;
    let instruction = physical.instruction()?;
    if resolved.get(usize::from(compiled.program_id_index)) != Some(&instruction.program_id)
        || compiled.data != instruction.data
        || compiled.accounts.len() != instruction.accounts.len()
        || compiled
            .accounts
            .iter()
            .zip(&instruction.accounts)
            .any(|(index, meta)| resolved.get(usize::from(*index)) != Some(&meta.pubkey))
    {
        return Err(refusal(
            "Series packet changed the selected generic-Hot instruction",
        ));
    }
    Ok(())
}

fn authenticate_finalization_v1(
    journal: &SeriesTerminalJournalV1,
    finalization: &SeriesTerminalFinalizationV1,
) -> Result<()> {
    let packet = journal
        .packet
        .as_ref()
        .ok_or_else(|| refusal("finalized Series journal omitted packet"))?;
    if finalization.signature != packet.signed.signature
        || finalization.packet_sha256 != packet.signed.packet_sha256
        || finalization.finalized_slot < journal.planner_finalized_slot
        || finalization.finalization_sha256 != finalization_digest_v1(finalization)?
        || journal.action.terminal() != finalization.complete_source_credit_lamports.is_some()
    {
        return Err(refusal("Series finalization evidence changed"));
    }
    require_sha256(
        &finalization.poststate_sha256,
        "Series finalization poststate",
    )?;
    Ok(())
}

fn authenticate_projection_v1(projection: &SeriesChainProjectionV1) -> Result<()> {
    require_sha256(
        &projection.ledger_identity_sha256,
        "Series projection ledger",
    )?;
    if projection.finalized_slot == 0
        || projection.accounts.is_empty()
        || projection.state_sha256 != projection_digest_v1(projection)?
    {
        return Err(refusal("Series projection digest or slot changed"));
    }
    for (key, account) in &projection.accounts {
        if key != &account.address {
            return Err(refusal("Series projection account map key changed"));
        }
        authenticate_durable_account_v1(account)?;
    }
    Ok(())
}

fn durable_present_account_v1(account: SeriesObservedAccountV1) -> Result<DurableSeriesAccountV1> {
    if account.lamports == 0 {
        return Err(refusal(
            "zero-lamport Series account must be represented as absent",
        ));
    }
    let data_base64 = BASE64.encode(&account.data);
    let mut value = DurableSeriesAccountV1 {
        address: account.key.to_string(),
        present: true,
        owner: Some(account.owner.to_string()),
        lamports: Some(account.lamports),
        executable: Some(account.executable),
        data_base64: Some(data_base64),
        data_sha256: Some(sha256_hex(&account.data)),
        account_sha256: String::new(),
    };
    value.account_sha256 = account_digest_v1(&value)?;
    Ok(value)
}

fn durable_absent_account_v1(key: Pubkey) -> DurableSeriesAccountV1 {
    let mut value = DurableSeriesAccountV1 {
        address: key.to_string(),
        present: false,
        owner: None,
        lamports: None,
        executable: None,
        data_base64: None,
        data_sha256: None,
        account_sha256: String::new(),
    };
    value.account_sha256 = account_digest_v1(&value).expect("serializable absent account");
    value
}

fn authenticate_durable_account_v1(account: &DurableSeriesAccountV1) -> Result<()> {
    parse_pubkey(&account.address, "Series durable account")?;
    let present_shape = if account.present {
        account.owner.is_some()
            && account.lamports.is_some_and(|value| value > 0)
            && account.executable.is_some()
            && account.data_base64.is_some()
            && account.data_sha256.is_some()
    } else {
        account.owner.is_none()
            && account.lamports.is_none()
            && account.executable.is_none()
            && account.data_base64.is_none()
            && account.data_sha256.is_none()
    };
    if !present_shape || account.account_sha256 != account_digest_v1(account)? {
        return Err(refusal("Series durable account shape or digest changed"));
    }
    if account.present {
        parse_pubkey(
            account.owner.as_deref().expect("present owner"),
            "Series account owner",
        )?;
        let data = decode_base64(
            account.data_base64.as_deref().expect("present data"),
            "Series account data",
        )?;
        if account.data_sha256.as_deref() != Some(sha256_hex(&data).as_str()) {
            return Err(refusal("Series durable account data digest changed"));
        }
    }
    Ok(())
}

fn require_account<'a>(
    projection: &'a SeriesChainProjectionV1,
    key: &str,
    label: &str,
) -> Result<&'a DurableSeriesAccountV1> {
    projection
        .accounts
        .get(key)
        .ok_or_else(|| refusal(format!("{label} was absent from the projection")))
}

fn require_present<'a>(
    projection: &'a SeriesChainProjectionV1,
    key: &str,
    label: &str,
) -> Result<&'a DurableSeriesAccountV1> {
    let account = require_account(projection, key, label)?;
    if !account.present {
        return Err(refusal(format!("{label} was absent")));
    }
    Ok(account)
}

fn require_same_account_except_lamports(
    before: &DurableSeriesAccountV1,
    after: &DurableSeriesAccountV1,
    label: &str,
) -> Result<()> {
    if !before.present
        || !after.present
        || before.address != after.address
        || before.owner != after.owner
        || before.executable != after.executable
        || before.data_base64 != after.data_base64
        || before.data_sha256 != after.data_sha256
    {
        return Err(refusal(format!("{label} changed beyond lamports")));
    }
    Ok(())
}

fn require_same_account_except_data(
    before: &DurableSeriesAccountV1,
    after: &DurableSeriesAccountV1,
    label: &str,
) -> Result<()> {
    if !before.present
        || !after.present
        || before.address != after.address
        || before.owner != after.owner
        || before.executable != after.executable
        || before.lamports != after.lamports
    {
        return Err(refusal(format!("{label} changed beyond its replay bytes")));
    }
    Ok(())
}

fn prepared_payer_v1(journal: &SeriesTerminalJournalV1) -> Result<String> {
    let payer = journal
        .payer
        .as_ref()
        .ok_or_else(|| refusal("prepared Series journal omitted its fee payer"))?;
    let prestate = journal
        .prestate
        .as_ref()
        .ok_or_else(|| refusal("prepared Series journal omitted prestate"))?;
    if !prestate.accounts.contains_key(payer)
        || journal
            .packet
            .as_ref()
            .is_some_and(|packet| packet.payer != *payer)
    {
        return Err(refusal(
            "Series durable fee payer changed or was unobserved",
        ));
    }
    Ok(payer.clone())
}

fn authenticate_ledger_identity_v1(identity: &SeriesLedgerIdentityV1) -> Result<()> {
    if !identity.canonical_ledger_path.starts_with('/')
        || identity.genesis_hash.is_empty()
        || identity.identity_sha256 != ledger_identity_digest_v1(identity)?
    {
        return Err(refusal("Series validator ledger identity changed"));
    }
    Ok(())
}

fn consequence_for_action(action: SeriesActionV3) -> SeriesConsequenceV3 {
    match action {
        SeriesActionV3::Prepare => SeriesConsequenceV3::PrepareTicket,
        SeriesActionV3::Consume => SeriesConsequenceV3::FoundOccurrenceMarket,
        SeriesActionV3::Expire => SeriesConsequenceV3::ExpireAndRefund,
        SeriesActionV3::Retire => SeriesConsequenceV3::RetireTicket,
        SeriesActionV3::Close => SeriesConsequenceV3::CloseRoot,
    }
}

fn consequence_text(value: SeriesConsequenceV3) -> &'static str {
    match value {
        SeriesConsequenceV3::PrepareTicket => "prepare-ticket",
        SeriesConsequenceV3::FoundOccurrenceMarket => "found-occurrence-market",
        SeriesConsequenceV3::ExpireAndRefund => "expire-and-refund",
        SeriesConsequenceV3::RetireTicket => "retire-ticket-to-lifecycle-rent-credit",
        SeriesConsequenceV3::CloseRoot => "close-root-to-lifecycle-rent-credit",
    }
}

fn kernel_action(value: SeriesJournalActionV1) -> SeriesActionV3 {
    match value {
        SeriesJournalActionV1::Prepare => SeriesActionV3::Prepare,
        SeriesJournalActionV1::Consume => SeriesActionV3::Consume,
        SeriesJournalActionV1::Expire => SeriesActionV3::Expire,
        SeriesJournalActionV1::Retire => SeriesActionV3::Retire,
        SeriesJournalActionV1::Close => SeriesActionV3::Close,
    }
}

fn refresh_journal_digest_v1(journal: &mut SeriesTerminalJournalV1) -> Result<()> {
    journal.state_sha256 = journal_state_digest_v1(journal)?;
    Ok(())
}

fn ledger_identity_digest_v1(value: &SeriesLedgerIdentityV1) -> Result<String> {
    let mut copy = value.clone();
    copy.identity_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn account_digest_v1(value: &DurableSeriesAccountV1) -> Result<String> {
    let mut copy = value.clone();
    copy.account_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn projection_digest_v1(value: &SeriesChainProjectionV1) -> Result<String> {
    let mut copy = value.clone();
    copy.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn authority_digest_v1(value: &DurableSeriesSelectedAuthorityV1) -> Result<String> {
    let mut copy = value.clone();
    copy.authority_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn physical_digest_v1(value: &DurableSeriesPhysicalActionV1) -> Result<String> {
    let mut copy = value.clone();
    copy.physical_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn packet_binding_digest_v1(value: &SeriesTerminalPacketBindingV1) -> Result<String> {
    let mut copy = value.clone();
    copy.packet_binding_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn finalization_digest_v1(value: &SeriesTerminalFinalizationV1) -> Result<String> {
    let mut copy = value.clone();
    copy.finalization_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn journal_intent_digest_v1(value: &SeriesTerminalJournalV1) -> Result<String> {
    let mut copy = value.clone();
    copy.phase = SeriesTerminalJournalPhaseV1::Planned;
    copy.physical = None;
    copy.payer = None;
    copy.prestate = None;
    copy.poststate = None;
    copy.packet = None;
    copy.finalization = None;
    copy.intent_sha256.clear();
    copy.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn journal_state_digest_v1(value: &SeriesTerminalJournalV1) -> Result<String> {
    let mut copy = value.clone();
    copy.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn conservation_receipt_digest_v1(value: &SeriesTerminalConservationReceiptV1) -> Result<String> {
    let mut copy = value.clone();
    copy.receipt_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn rollback_receipt_digest_v1(value: &SeriesTerminalRollbackReceiptV1) -> Result<String> {
    let mut copy = value.clone();
    copy.receipt_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn market_retirement_binding_digest_v1(value: &SeriesMarketRetirementBindingV1) -> Result<String> {
    let mut value = value.clone();
    value.binding_sha256.clear();
    Ok(sha256_hex(&canonical_series_json_v1(&value)?))
}

fn found_binding_digest_v1(value: &SeriesFoundBindingV1) -> Result<String> {
    let mut copy = value.clone();
    copy.binding_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn complete_lifecycle_digest_v1(value: &SeriesCompleteLifecycleLedgerV1) -> Result<String> {
    let mut copy = value.clone();
    copy.ledger_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn require_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(refusal(format!("{label} was not lowercase SHA-256")));
    }
    Ok(())
}

fn decode_base64(value: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    if BASE64.encode(&bytes) != value {
        return Err(refusal(format!("{label} was not canonical base64")));
    }
    Ok(bytes)
}

fn pubkey_bytes(values: &[Pubkey]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_bytes()).collect()
}

fn hex32(value: [u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(format!("REFUSED Series terminal: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use dclutch_trading_sbf::TradingSbfError;
    use solana_program::instruction::AccountMeta;
    use solana_sdk::{
        message::{VersionedMessage, v0},
        signature::{Keypair, Signer},
        transaction::VersionedTransaction,
    };

    use super::*;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn acquisition_recipe(sequence: u32) -> SeriesHotAcquisitionRecipeV2 {
        let record = |raw: u8, staging: u8| SeriesFinalizedRecordAddressesV2 {
            raw: key(raw).to_string(),
            staging: key(staging).to_string(),
        };
        SeriesHotAcquisitionRecipeV2 {
            sequence,
            fixed: SeriesHotFixedAddressesV2 {
                market: key(1).to_string(),
                root: key(2).to_string(),
                manifest: record(3, 4),
                program_set: record(5, 6),
                descriptor: record(7, 8),
                config: record(9, 10),
                account_profile: record(11, 12),
                request_profile: record(13, 14),
                transition: record(15, 16),
                effect: record(17, 18),
                lifecycle: record(19, 20),
                strategy: record(21, 22),
                activation_cache: key(23).to_string(),
                core_program: key(24).to_string(),
                core_programdata: key(25).to_string(),
                trading_program: key(26).to_string(),
                trading_programdata: key(27).to_string(),
                registry_program: key(28).to_string(),
                rent_sysvar: key(29).to_string(),
                instructions_sysvar: key(30).to_string(),
                product: record(31, 32),
                result_domain: record(33, 34),
                portfolio: record(35, 36),
                linked_basis: record(37, 38),
                capability_seal: key(39).to_string(),
            },
            runtime_logical_accounts: vec![key(40).to_string()],
            consume_shadow: None,
            current_occurrence: None,
            terminal_ticket: None,
            lifecycle_rent_credit: None,
            expire_permit: None,
        }
    }

    fn request(action: SeriesActionV3, ticket: Option<[u8; 32]>, revision: u64) -> Vec<u8> {
        action_request(
            action,
            action.occurrence_bound().then_some([3; 32]),
            ticket,
            revision,
        )
    }

    fn action_request(
        action: SeriesActionV3,
        occurrence: Option<[u8; 32]>,
        ticket: Option<[u8; 32]>,
        revision: u64,
    ) -> Vec<u8> {
        let mut bytes = vec![0_u8; 128];
        bytes[..8].copy_from_slice(b"DCLTSIX3");
        bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12] = match action {
            SeriesActionV3::Prepare => 0,
            SeriesActionV3::Consume => 1,
            SeriesActionV3::Expire => 2,
            SeriesActionV3::Retire => 3,
            SeriesActionV3::Close => 4,
        };
        bytes[16..48].copy_from_slice(&[1; 32]);
        if let Some(occurrence) = occurrence {
            bytes[48..80].copy_from_slice(&occurrence);
        }
        if let Some(ticket) = ticket {
            bytes[80..112].copy_from_slice(&ticket);
        }
        bytes[112..120].copy_from_slice(&revision.to_le_bytes());
        if !matches!(action, SeriesActionV3::Prepare | SeriesActionV3::Close) {
            bytes[120..128].copy_from_slice(&7_u64.to_le_bytes());
        }
        SeriesActionRequestV3::decode(&bytes).expect("canonical test request");
        bytes
    }

    fn ledger() -> SeriesLedgerIdentityV1 {
        SeriesLedgerIdentityV1::admit("/tmp/dclutch-existing-ledger".into(), key(90).to_string())
            .expect("ledger")
    }

    fn market_retirement(
        market: Pubkey,
        generation: u64,
        tag: u8,
        finalized_slot: u64,
    ) -> SeriesMarketRetirementBindingV1 {
        let mut value = SeriesMarketRetirementBindingV1 {
            ledger_identity_sha256: ledger().identity_sha256,
            selected_release_set: hex32([21; 32]),
            market: market.to_string(),
            generation,
            rent_credit: key(12).to_string(),
            core_program: key(41).to_string(),
            claims_program: key(42).to_string(),
            aggregate_campaign_sha256: format!("{tag:02x}").repeat(32),
            aggregate_completion_sha256: format!("{:02x}", tag + 16).repeat(32),
            finalized_slot,
            total_transaction_fees_lamports: 20,
            total_compute_units_consumed: 400_000,
            binding_sha256: String::new(),
        };
        value.binding_sha256 =
            market_retirement_binding_digest_v1(&value).expect("retirement binding digest");
        authenticate_series_market_retirement_binding_v1(&value).expect("retirement binding shape");
        value
    }

    fn plan(action: SeriesActionV3) -> SeriesTerminalJournalV1 {
        plan_decoded_series_journal_v1(
            SeriesPlannerObservationV1 {
                campaign_sha256: "11".repeat(32),
                ledger: ledger(),
                finalized_slot: 40,
                snapshot_sha256: "44".repeat(32),
            },
            3,
            2,
            if action == SeriesActionV3::Close {
                0
            } else {
                1
            },
            action,
            consequence_for_action(action),
            &request(
                action,
                (action == SeriesActionV3::Retire).then_some([2; 32]),
                9,
            ),
        )
        .expect("planned")
    }

    #[derive(Clone)]
    struct Selected {
        action: SeriesActionV3,
        request: Vec<u8>,
        instruction: Instruction,
        mechanism: SeriesPhysicalMechanismV1,
        roles: SeriesPhysicalRoleKeysV1,
        observation_slot: u64,
    }

    impl SelectedSeriesPhysicalActionV1 for Selected {
        fn canonical_request_bytes(&self) -> &[u8] {
            &self.request
        }

        fn action(&self) -> SeriesActionV3 {
            self.action
        }

        fn observation(&self) -> Observation {
            Observation {
                slot: self.observation_slot,
                unix_timestamp: 0,
                finality: dclutch_operator::Finality::Finalized,
            }
        }

        fn trading_program(&self) -> Pubkey {
            key(20)
        }

        fn generic_hot_instruction(&self) -> Instruction {
            self.instruction.clone()
        }

        fn selected_authority_ids(&self) -> SeriesSelectedAuthorityIdsV1 {
            SeriesSelectedAuthorityIdsV1 {
                release_set: [21; 32],
                program_set_v2: [22; 32],
                descriptor: [23; 32],
                profile_v3: [24; 32],
                request_profile: [25; 32],
                lifecycle_policy: [26; 32],
                strategy: [27; 32],
                transition: [28; 32],
                effect_v5: [29; 32],
            }
        }

        fn role_keys(&self) -> SeriesPhysicalRoleKeysV1 {
            self.roles
        }

        fn mechanism(&self) -> SeriesPhysicalMechanismV1 {
            self.mechanism
        }

        fn consequence(&self) -> SeriesConsequenceV3 {
            consequence_for_action(self.action)
        }
    }

    fn selected(action: SeriesActionV3) -> Selected {
        let root = key(10);
        let ticket = (action == SeriesActionV3::Retire).then_some(key(11));
        let credit = action.then_terminal().then_some(key(12));
        let occurrence_market = action.occurrence_bound().then_some(key(31));
        let occurrence_permit = (action == SeriesActionV3::Expire).then_some(key(32));
        let mut accounts = vec![
            AccountMeta::new(root, false),
            AccountMeta::new_readonly(key(9), false),
        ];
        if let Some(ticket) = ticket {
            accounts.push(AccountMeta::new(ticket, false));
        }
        if let Some(credit) = credit {
            accounts.push(AccountMeta::new(credit, false));
        }
        if let Some(market) = occurrence_market {
            accounts.push(AccountMeta::new(market, false));
        }
        if let Some(permit) = occurrence_permit {
            accounts.push(AccountMeta::new(permit, false));
        }
        accounts.push(AccountMeta::new_readonly(key(30), false));
        Selected {
            action,
            request: request(
                action,
                (action == SeriesActionV3::Retire).then_some([2; 32]),
                9,
            ),
            instruction: Instruction {
                program_id: key(20),
                accounts,
                data: [b"DCLTHOT3".as_slice(), &[action as u8]].concat(),
            },
            mechanism: match action {
                SeriesActionV3::Retire => SeriesPhysicalMechanismV1::RetireFundingV5Close,
                SeriesActionV3::Close => SeriesPhysicalMechanismV1::CloseLifecycleOnlyRoot,
                _ => SeriesPhysicalMechanismV1::Occurrence,
            },
            roles: SeriesPhysicalRoleKeysV1 {
                root,
                ticket,
                rent_credit: credit,
                parent_market: key(9),
                parent_market_generation: 1,
                occurrence_market,
                occurrence_market_generation: occurrence_market.map(|_| 1),
                occurrence_permit,
            },
            observation_slot: 40,
        }
    }

    trait TerminalAction {
        fn then_terminal(self) -> bool;
    }

    impl TerminalAction for SeriesActionV3 {
        fn then_terminal(self) -> bool {
            matches!(self, Self::Retire | Self::Close)
        }
    }

    fn observed(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: &[u8],
    ) -> SeriesObservedAccountSlotV1 {
        SeriesObservedAccountSlotV1 {
            key,
            account: Some(SeriesObservedAccountV1 {
                key,
                owner,
                lamports,
                executable: false,
                data: data.to_vec(),
            }),
        }
    }

    fn absent(key: Pubkey) -> SeriesObservedAccountSlotV1 {
        SeriesObservedAccountSlotV1 { key, account: None }
    }

    fn projection(
        action: SeriesActionV3,
        slot: u64,
        ticket_lamports: Option<u64>,
        root_lamports: Option<u64>,
        credit_lamports: u64,
        payer_lamports: u64,
        root_data: &[u8],
    ) -> SeriesChainProjectionV1 {
        let mut accounts = Vec::new();
        match root_lamports {
            Some(value) => accounts.push(observed(key(10), key(20), value, root_data)),
            None => accounts.push(absent(key(10))),
        }
        accounts.push(observed(key(9), key(41), 500, &[8]));
        if action == SeriesActionV3::Retire {
            match ticket_lamports {
                Some(value) => accounts.push(observed(key(11), key(20), value, &[2])),
                None => accounts.push(absent(key(11))),
            }
        }
        accounts.push(observed(key(12), key(40), credit_lamports, &[3]));
        accounts.push(observed(
            key(13),
            solana_sdk_ids::system_program::ID,
            payer_lamports,
            &[],
        ));
        build_series_chain_projection_v1(&ledger(), slot, accounts).expect("projection")
    }

    fn prepared(action: SeriesActionV3) -> SeriesTerminalJournalV1 {
        let prestate = projection(action, 40, Some(55), Some(100), 1_000, 10_000, &[1]);
        prepare_series_terminal_journal_v1(&plan(action), &selected(action), prestate, key(13))
            .expect("prepared")
    }

    fn packet(
        prepared: &SeriesTerminalJournalV1,
        payer: &Keypair,
    ) -> SeriesTerminalPacketBindingV1 {
        let instruction = prepared
            .physical
            .as_ref()
            .expect("physical")
            .instruction()
            .expect("instruction");
        let message = v0::Message::try_compile(
            &payer.pubkey(),
            std::slice::from_ref(&instruction),
            &[],
            solana_hash::Hash::new_unique(),
        )
        .expect("v0 message");
        let transaction = VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            std::slice::from_ref(payer),
        )
        .expect("signed transaction");
        let resolved = transaction.message.static_account_keys().to_vec();
        let bytes = bincode::serialize(&transaction).expect("packet bytes");
        build_series_terminal_packet_binding_v1(
            prepared,
            SignedVersionedPacketV1 {
                signature: transaction.signatures[0].to_string(),
                packet_base64: BASE64.encode(&bytes),
                packet_sha256: sha256_hex(&bytes),
                last_valid_block_height: 500,
            },
            payer.pubkey(),
            key(50),
            "55".repeat(32),
            resolved,
        )
        .expect("packet binding")
    }

    fn submitted(action: SeriesActionV3) -> (SeriesTerminalJournalV1, Keypair) {
        let mut value = prepared(action);
        let payer = Keypair::new();
        // The journal's exact prestate names key(13), so the signed fee payer
        // must do so too. `Keypair::new` cannot satisfy that. Rebuild prestate
        // with the actual payer while preserving every protocol fact.
        let prestate = value.prestate.as_ref().expect("prestate");
        let mut accounts = prestate.accounts.clone();
        accounts.remove(&key(13).to_string());
        let payer_account = durable_present_account_v1(SeriesObservedAccountV1 {
            key: payer.pubkey(),
            owner: solana_sdk_ids::system_program::ID,
            lamports: 10_000,
            executable: false,
            data: Vec::new(),
        })
        .expect("payer account");
        accounts.insert(payer.pubkey().to_string(), payer_account);
        let mut rewritten = SeriesChainProjectionV1 {
            ledger_identity_sha256: prestate.ledger_identity_sha256.clone(),
            finalized_slot: prestate.finalized_slot,
            accounts,
            state_sha256: String::new(),
        };
        rewritten.state_sha256 = projection_digest_v1(&rewritten).expect("projection digest");
        let physical = value.physical.clone();
        value = plan(action);
        value.phase = SeriesTerminalJournalPhaseV1::Prepared;
        value.physical = physical;
        value.payer = Some(payer.pubkey().to_string());
        value.prestate = Some(rewritten);
        refresh_journal_digest_v1(&mut value).expect("journal digest");
        authenticate_series_terminal_journal_v1(&value).expect("prepared with actual payer");
        let binding = packet(&value, &payer);
        let dispatching =
            dispatch_series_terminal_journal_v1(&value, binding).expect("dispatching");
        let signature = dispatching
            .packet
            .as_ref()
            .expect("packet")
            .signed
            .signature
            .clone();
        let submitted =
            submit_series_terminal_journal_v1(&dispatching, &signature).expect("submitted");
        (submitted, payer)
    }

    fn terminal_poststate(
        submitted: &SeriesTerminalJournalV1,
        action: SeriesActionV3,
        payer: Pubkey,
        exact_credit: u64,
    ) -> SeriesChainProjectionV1 {
        let mut accounts = Vec::new();
        if action == SeriesActionV3::Retire {
            accounts.push(observed(key(10), key(20), 100, &[9]));
            accounts.push(absent(key(11)));
        } else {
            accounts.push(absent(key(10)));
        }
        accounts.push(observed(key(9), key(41), 500, &[8]));
        accounts.push(observed(key(12), key(40), exact_credit, &[3]));
        accounts.push(observed(
            payer,
            solana_sdk_ids::system_program::ID,
            9_995,
            &[],
        ));
        build_series_chain_projection_v1(&submitted.ledger, 41, accounts).expect("poststate")
    }

    #[allow(clippy::too_many_arguments)]
    fn lifecycle_finalized_action(
        action: SeriesActionV3,
        sequence: u32,
        next_occurrence: u32,
        outstanding: u32,
        occurrence_byte: u8,
        ticket_byte: u8,
        payer: &Keypair,
        root_before_data: u8,
        root_after_data: u8,
        ticket_before_data: Option<u8>,
        ticket_after_data: Option<u8>,
        credit_before: Option<u64>,
    ) -> (
        SeriesTerminalJournalV1,
        Option<SeriesTerminalConservationReceiptV1>,
    ) {
        let planner_slot = 81_u64 + u64::from(sequence) * 2;
        let finalized_slot = planner_slot + 1;
        let occurrence = action.occurrence_bound().then_some([occurrence_byte; 32]);
        let ticket = (action != SeriesActionV3::Close).then_some([ticket_byte; 32]);
        let request = action_request(action, occurrence, ticket, u64::from(sequence) + 1);
        let planned = plan_decoded_series_journal_v1(
            SeriesPlannerObservationV1 {
                campaign_sha256: "11".repeat(32),
                ledger: ledger(),
                finalized_slot: planner_slot,
                snapshot_sha256: "44".repeat(32),
            },
            sequence,
            next_occurrence,
            outstanding,
            action,
            consequence_for_action(action),
            &request,
        )
        .expect("lifecycle planned action");
        let root = key(10);
        let ticket_account = (action != SeriesActionV3::Close).then_some(key(100 + ticket_byte));
        let rent_credit = action.then_terminal().then_some(key(12));
        let occurrence_market = action
            .occurrence_bound()
            .then_some(key(60 + occurrence_byte));
        let occurrence_permit =
            (action == SeriesActionV3::Expire).then_some(key(70 + occurrence_byte));
        let mut metas = vec![
            AccountMeta::new(root, false),
            AccountMeta::new_readonly(key(9), false),
        ];
        if let Some(ticket_account) = ticket_account {
            metas.push(AccountMeta::new(ticket_account, false));
        }
        if let Some(rent_credit) = rent_credit {
            metas.push(AccountMeta::new(rent_credit, false));
        }
        if let Some(market) = occurrence_market {
            metas.push(AccountMeta::new(market, false));
        }
        if let Some(permit) = occurrence_permit {
            metas.push(AccountMeta::new(permit, false));
        }
        metas.push(AccountMeta::new_readonly(key(30), false));
        let selected = Selected {
            action,
            request,
            instruction: Instruction {
                program_id: key(20),
                accounts: metas,
                data: [b"DCLTHOT3".as_slice(), &[action as u8]].concat(),
            },
            mechanism: match action {
                SeriesActionV3::Retire => SeriesPhysicalMechanismV1::RetireFundingV5Close,
                SeriesActionV3::Close => SeriesPhysicalMechanismV1::CloseLifecycleOnlyRoot,
                _ => SeriesPhysicalMechanismV1::Occurrence,
            },
            roles: SeriesPhysicalRoleKeysV1 {
                root,
                ticket: ticket_account,
                rent_credit,
                parent_market: key(9),
                parent_market_generation: 1,
                occurrence_market,
                occurrence_market_generation: occurrence_market.map(|_| 1),
                occurrence_permit,
            },
            observation_slot: planner_slot,
        };
        let payer_before = 10_000_u64 - u64::from(sequence) * 5;
        let ticket_lamports = 50_u64 + u64::from(ticket_byte);
        let mut before = vec![observed(root, key(20), 100, &[root_before_data])];
        before.push(observed(key(9), key(41), 500, &[8]));
        if let Some(ticket_account) = ticket_account {
            match ticket_before_data {
                Some(data) => {
                    before.push(observed(ticket_account, key(20), ticket_lamports, &[data]))
                }
                None => before.push(absent(ticket_account)),
            }
        }
        if let Some(credit) = credit_before {
            before.push(observed(key(12), key(40), credit, &[3]));
        }
        if let Some(market) = occurrence_market {
            before.push(absent(market));
        }
        if let Some(permit) = occurrence_permit {
            before.push(absent(permit));
        }
        before.push(observed(
            payer.pubkey(),
            solana_sdk_ids::system_program::ID,
            payer_before,
            &[],
        ));
        let prestate = build_series_chain_projection_v1(&ledger(), planner_slot, before)
            .expect("lifecycle prestate");
        let prepared =
            prepare_series_terminal_journal_v1(&planned, &selected, prestate, payer.pubkey())
                .expect("lifecycle prepared");
        let dispatching = dispatch_series_terminal_journal_v1(&prepared, packet(&prepared, payer))
            .expect("lifecycle dispatching");
        let signature = dispatching
            .packet
            .as_ref()
            .expect("lifecycle packet")
            .signed
            .signature
            .clone();
        let submitted = submit_series_terminal_journal_v1(&dispatching, &signature)
            .expect("lifecycle submitted");
        let mut after = Vec::new();
        if action == SeriesActionV3::Close {
            after.push(absent(root));
        } else {
            after.push(observed(root, key(20), 100, &[root_after_data]));
        }
        after.push(observed(key(9), key(41), 500, &[8]));
        if let Some(ticket_account) = ticket_account {
            match ticket_after_data {
                Some(data) => {
                    after.push(observed(ticket_account, key(20), ticket_lamports, &[data]))
                }
                None => after.push(absent(ticket_account)),
            }
        }
        if let Some(credit) = credit_before {
            let source = if action == SeriesActionV3::Close {
                100
            } else {
                ticket_lamports
            };
            after.push(observed(key(12), key(40), credit + source, &[3]));
        }
        if let Some(market) = occurrence_market {
            if action == SeriesActionV3::Consume {
                after.push(observed(market, key(42), 600, &[6]));
            } else {
                after.push(absent(market));
            }
        }
        if let Some(permit) = occurrence_permit {
            after.push(absent(permit));
        }
        after.push(observed(
            payer.pubkey(),
            solana_sdk_ids::system_program::ID,
            payer_before - 5,
            &[],
        ));
        let poststate =
            build_series_chain_projection_v1(&ledger(), finalized_slot, after).expect("poststate");
        let packet = submitted.packet.as_ref().expect("submitted packet");
        finalize_series_terminal_journal_v1(
            &submitted,
            packet.signed.signature.clone(),
            packet.signed.packet_sha256.clone(),
            5,
            500_000 + u64::from(sequence),
            poststate,
        )
        .expect("lifecycle finalized")
    }

    #[test]
    fn retire_runs_every_durable_phase_and_proves_complete_ticket_credit() {
        let (submitted, payer) = submitted(SeriesActionV3::Retire);
        assert_eq!(
            series_terminal_recovery_v1(&submitted).expect("recovery"),
            SeriesTerminalRecoveryV1::PollOnly
        );
        let packet = submitted.packet.as_ref().expect("packet");
        let poststate =
            terminal_poststate(&submitted, SeriesActionV3::Retire, payer.pubkey(), 1_055);
        let (finalized, receipt) = finalize_series_terminal_journal_v1(
            &submitted,
            packet.signed.signature.clone(),
            packet.signed.packet_sha256.clone(),
            5,
            700_000,
            poststate,
        )
        .expect("finalized Retire");
        let receipt = receipt.expect("terminal conservation");
        assert_eq!(receipt.source_lamports_before, 55);
        assert_eq!(receipt.rent_credit_lamports_after, 1_055);
        assert!(receipt.donation_inclusive_exact_credit);
        assert_eq!(
            series_terminal_recovery_v1(&finalized).expect("complete"),
            SeriesTerminalRecoveryV1::Complete
        );
    }

    #[test]
    fn close_proves_complete_root_credit_and_no_ticket_role() {
        let (submitted, payer) = submitted(SeriesActionV3::Close);
        let packet = submitted.packet.as_ref().expect("packet");
        let poststate =
            terminal_poststate(&submitted, SeriesActionV3::Close, payer.pubkey(), 1_100);
        let (_, receipt) = finalize_series_terminal_journal_v1(
            &submitted,
            packet.signed.signature.clone(),
            packet.signed.packet_sha256.clone(),
            5,
            600_000,
            poststate,
        )
        .expect("finalized Close");
        assert_eq!(receipt.expect("receipt").source_lamports_before, 100);
    }

    #[test]
    fn native_completion_requires_two_paths_once_only_retire_and_chain_continuity() {
        let payer = Keypair::new();
        let (prepare_consume, _) = lifecycle_finalized_action(
            SeriesActionV3::Prepare,
            0,
            0,
            0,
            1,
            1,
            &payer,
            0,
            1,
            None,
            Some(10),
            None,
        );
        let (consume, _) = lifecycle_finalized_action(
            SeriesActionV3::Consume,
            1,
            0,
            1,
            1,
            1,
            &payer,
            1,
            2,
            Some(10),
            Some(11),
            None,
        );
        let (retire_consumed, retire_consumed_receipt) = lifecycle_finalized_action(
            SeriesActionV3::Retire,
            2,
            1,
            1,
            1,
            1,
            &payer,
            2,
            3,
            Some(11),
            None,
            Some(1_000),
        );
        let (prepare_expire, _) = lifecycle_finalized_action(
            SeriesActionV3::Prepare,
            3,
            1,
            0,
            2,
            2,
            &payer,
            3,
            4,
            None,
            Some(20),
            None,
        );
        let (expire, _) = lifecycle_finalized_action(
            SeriesActionV3::Expire,
            4,
            1,
            1,
            2,
            2,
            &payer,
            4,
            5,
            Some(20),
            Some(21),
            None,
        );
        let (retire_expired, retire_expired_receipt) = lifecycle_finalized_action(
            SeriesActionV3::Retire,
            5,
            2,
            1,
            2,
            2,
            &payer,
            5,
            6,
            Some(21),
            None,
            Some(1_051),
        );
        let (close, close_receipt) = lifecycle_finalized_action(
            SeriesActionV3::Close,
            6,
            2,
            0,
            0,
            0,
            &payer,
            6,
            0,
            None,
            None,
            Some(1_103),
        );
        let journals = vec![
            prepare_consume,
            consume,
            retire_consumed,
            prepare_expire,
            expire,
            retire_expired,
            close,
        ];
        let conservation = vec![
            retire_consumed_receipt.expect("consumed Ticket receipt"),
            retire_expired_receipt.expect("expired Ticket receipt"),
            close_receipt.expect("root receipt"),
        ];
        let found = admit_series_found_binding_v1(
            "11".repeat(32),
            &ledger(),
            key(10),
            key(9),
            1,
            [1; 32],
            Signature::new_unique().to_string(),
            80,
            "aa".repeat(32),
            "bb".repeat(32),
        )
        .expect("Found binding");
        let market_retirements = vec![
            market_retirement(key(61), 1, 0x81, 100),
            market_retirement(key(9), 1, 0x82, 101),
        ];
        let completion = build_series_complete_lifecycle_ledger_v1(
            found.clone(),
            ledger(),
            &journals,
            &conservation,
            &market_retirements,
        )
        .expect("native Series completion");
        assert_eq!(completion.occurrence_count, 2);
        assert_eq!(completion.consumed_occurrences, 1);
        assert_eq!(completion.expired_occurrences, 1);
        assert_eq!(completion.total_terminal_credit_lamports, 203);
        assert!(completion.temporary_protocol_state_closed);
        let consumed_completion = completion
            .occurrence_completions
            .iter()
            .find(|value| value.settlement == SeriesJournalActionV1::Consume)
            .expect("consumed occurrence");
        let expired_completion = completion
            .occurrence_completions
            .iter()
            .find(|value| value.settlement == SeriesJournalActionV1::Expire)
            .expect("expired occurrence");
        assert_eq!(consumed_completion.future_market, key(61).to_string());
        assert_eq!(expired_completion.future_market, key(62).to_string());
        assert_ne!(
            consumed_completion.future_market,
            expired_completion.future_market
        );
        let expected_expire_permit = key(72).to_string();
        assert_eq!(
            expired_completion.expire_permit.as_deref(),
            Some(expected_expire_permit.as_str())
        );
        assert_eq!(
            expired_completion
                .expire_vacancy_poststate_sha256
                .as_deref(),
            completion
                .actions
                .iter()
                .find(|action| action.action == SeriesJournalActionV1::Expire)
                .map(|action| action.poststate_sha256.as_str())
        );

        let mut controller_substitution = completion.clone();
        controller_substitution
            .occurrence_completions
            .iter_mut()
            .find(|value| value.settlement == SeriesJournalActionV1::Expire)
            .expect("expired occurrence")
            .future_market = found.parent_market.clone();
        controller_substitution.ledger_sha256 =
            complete_lifecycle_digest_v1(&controller_substitution).expect("completion digest");
        let error = authenticate_series_complete_lifecycle_ledger_v1(&controller_substitution)
            .expect_err("controller/future substitution must refuse");
        assert!(
            error.to_string().contains("controller separation changed"),
            "unexpected controller substitution refusal: {error}"
        );

        let mut reversed_retirements = market_retirements.clone();
        reversed_retirements.reverse();
        assert!(
            build_series_complete_lifecycle_ledger_v1(
                found.clone(),
                ledger(),
                &journals,
                &conservation,
                &reversed_retirements,
            )
            .unwrap_err()
            .to_string()
            .contains("child Market retirement")
        );

        let mut fabricated_child = completion.clone();
        let expired = fabricated_child
            .occurrence_completions
            .iter_mut()
            .find(|value| value.settlement == SeriesJournalActionV1::Expire)
            .expect("expired occurrence");
        expired.child_market = Some(key(62).to_string());
        fabricated_child.ledger_sha256 =
            complete_lifecycle_digest_v1(&fabricated_child).expect("completion digest");
        assert!(
            authenticate_series_complete_lifecycle_ledger_v1(&fabricated_child)
                .unwrap_err()
                .to_string()
                .contains("fabricated")
        );

        let mut unused = conservation.clone();
        unused.push(conservation[0].clone());
        assert!(
            build_series_complete_lifecycle_ledger_v1(
                found.clone(),
                ledger(),
                &journals,
                &unused,
                &market_retirements,
            )
            .unwrap_err()
            .to_string()
            .contains("source uniqueness")
        );

        let mut discontinuous = journals.clone();
        let prestate = discontinuous[3].prestate.as_mut().expect("prestate");
        let root = prestate
            .accounts
            .get_mut(&key(10).to_string())
            .expect("root");
        root.lamports = Some(101);
        root.account_sha256 = account_digest_v1(root).expect("account digest");
        prestate.state_sha256 = projection_digest_v1(prestate).expect("projection digest");
        refresh_journal_digest_v1(&mut discontinuous[3]).expect("journal digest");
        assert!(
            build_series_complete_lifecycle_ledger_v1(
                found,
                ledger(),
                &discontinuous,
                &conservation,
                &market_retirements,
            )
            .unwrap_err()
            .to_string()
            .contains("did not continue")
        );
    }

    #[test]
    fn mechanism_substitution_and_one_lamport_conservation_drift_refuse() {
        let mut wrong = selected(SeriesActionV3::Retire);
        wrong.mechanism = SeriesPhysicalMechanismV1::Occurrence;
        let prestate = projection(
            SeriesActionV3::Retire,
            40,
            Some(55),
            Some(100),
            1_000,
            10_000,
            &[1],
        );
        assert!(
            prepare_series_terminal_journal_v1(
                &plan(SeriesActionV3::Retire),
                &wrong,
                prestate,
                key(13)
            )
            .unwrap_err()
            .to_string()
            .contains("exact V5 terminal mechanism")
        );

        let (submitted, payer) = submitted(SeriesActionV3::Retire);
        let packet = submitted.packet.as_ref().expect("packet");
        let short = terminal_poststate(&submitted, SeriesActionV3::Retire, payer.pubkey(), 1_054);
        assert!(
            finalize_series_terminal_journal_v1(
                &submitted,
                packet.signed.signature.clone(),
                packet.signed.packet_sha256.clone(),
                5,
                1,
                short,
            )
            .unwrap_err()
            .to_string()
            .contains("complete observed balance")
        );
    }

    #[test]
    fn exact_custom_refusal_proves_rollback_and_detects_one_lamport_drift() {
        let exact_refusal = TradingSbfError::Content as u32;
        let prepared = prepared(SeriesActionV3::Retire);
        let mut rollback = prepared.prestate.as_ref().expect("prestate").clone();
        rollback.finalized_slot = 41;
        let payer = key(13).to_string();
        let payer_before = require_present(&rollback, &payer, "payer")
            .expect("payer")
            .lamports
            .expect("payer lamports");
        let payer_after = durable_present_account_v1(SeriesObservedAccountV1 {
            key: key(13),
            owner: solana_sdk_ids::system_program::ID,
            lamports: payer_before - 5,
            executable: false,
            data: Vec::new(),
        })
        .expect("payer after");
        rollback.accounts.insert(payer.clone(), payer_after);
        rollback.state_sha256 = projection_digest_v1(&rollback).expect("rollback digest");
        let signature = Signature::new_unique().to_string();
        let receipt = prove_series_terminal_rollback_v1(
            &prepared,
            signature.clone(),
            "aa".repeat(32),
            exact_refusal,
            exact_refusal,
            5,
            99,
            rollback.clone(),
        )
        .expect("exact rollback");
        assert!(receipt.protocol_accounts_byte_and_lamport_exact);

        let mut drift = rollback.clone();
        let ticket = drift
            .accounts
            .get_mut(&key(11).to_string())
            .expect("ticket");
        ticket.lamports = Some(ticket.lamports.expect("lamports") - 1);
        ticket.account_sha256 = account_digest_v1(ticket).expect("account digest");
        drift.state_sha256 = projection_digest_v1(&drift).expect("projection digest");
        assert!(
            prove_series_terminal_rollback_v1(
                &prepared,
                signature.clone(),
                "aa".repeat(32),
                exact_refusal,
                exact_refusal,
                5,
                99,
                drift,
            )
            .unwrap_err()
            .to_string()
            .contains("roll protocol bytes and lamports back exactly")
        );
        assert!(
            prove_series_terminal_rollback_v1(
                &prepared,
                signature,
                "aa".repeat(32),
                exact_refusal,
                TradingSbfError::Transition as u32,
                5,
                99,
                rollback,
            )
            .unwrap_err()
            .to_string()
            .contains("exact finalized custom refusal")
        );
    }

    #[test]
    fn phase_skip_and_planner_observation_substitution_refuse() {
        let planned = plan(SeriesActionV3::Close);
        assert!(
            create_series_terminal_journal_file_v1(Path::new("/must-not-be-created"), &planned)
                .unwrap_err()
                .to_string()
                .contains("first durable Series journal must be same-slot Prepared")
        );
        let mut skipped = planned.clone();
        skipped.phase = SeriesTerminalJournalPhaseV1::Submitted;
        refresh_journal_digest_v1(&mut skipped).expect("digest");
        assert!(authenticate_series_terminal_journal_v1(&skipped).is_err());

        let mut wrong_observation = selected(SeriesActionV3::Close);
        wrong_observation.request[112] ^= 1;
        let prestate = projection(
            SeriesActionV3::Close,
            40,
            None,
            Some(100),
            1_000,
            10_000,
            &[1],
        );
        assert!(
            prepare_series_terminal_journal_v1(&planned, &wrong_observation, prestate, key(13))
                .unwrap_err()
                .to_string()
                .contains("changed the planner request")
        );
    }

    #[test]
    fn prepared_is_the_create_new_fsync_boundary() {
        let prepared = prepared(SeriesActionV3::Close);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "dclutch-series-terminal-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("test directory");
        let path = directory.join("0003.json");
        create_series_terminal_journal_file_v1(&path, &prepared).expect("first durable Prepared");
        assert_eq!(
            read_series_terminal_journal_file_v1(&path).expect("read durable Prepared"),
            prepared
        );
        assert!(create_series_terminal_journal_file_v1(&path, &prepared).is_err());
        fs::remove_file(&path).expect("remove test journal");
        fs::remove_dir(&directory).expect("remove test directory");
    }

    #[test]
    fn only_current_canonical_acquisition_becomes_a_durable_address_frame() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "dclutch-series-acquired-frame-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("test directory");
        let recipe = acquisition_recipe(3);
        let selected = selected(SeriesActionV3::Close);
        let first = load_or_create_series_acquired_address_frame_v2(
            &directory,
            3,
            &recipe,
            &"11".repeat(32),
            &ledger(),
            &selected,
        )
        .expect("current acquired frame");
        assert_eq!(first.recipe, recipe);
        assert_eq!(first.action, SeriesJournalActionV1::Close);
        assert!(series_acquired_address_frame_path_v2(&directory, 3).is_file());
        assert!(!series_acquired_address_frame_path_v2(&directory, 4).exists());
        let reread = load_or_create_series_acquired_address_frame_v2(
            &directory,
            3,
            &recipe,
            &"11".repeat(32),
            &ledger(),
            &selected,
        )
        .expect("idempotent reread");
        assert_eq!(reread, first);
        authenticate_series_acquired_address_frame_history_v2(
            &directory,
            std::slice::from_ref(&recipe),
            &[],
            3,
            &"11".repeat(32),
            &ledger(),
        )
        .expect("restart byte-matches current frame before acquisition");

        let mut advanced_observation = selected.clone();
        advanced_observation.observation_slot += 1;
        assert!(
            load_or_create_series_acquired_address_frame_v2(
                &directory,
                3,
                &recipe,
                &"11".repeat(32),
                &ledger(),
                &advanced_observation,
            )
            .err()
            .expect("historical frame must not move to a later observation")
            .to_string()
            .contains("canonical current observation")
        );
        let future_recipe = acquisition_recipe(4);
        assert!(
            load_or_create_series_acquired_address_frame_v2(
                &directory,
                3,
                &future_recipe,
                &"11".repeat(32),
                &ledger(),
                &selected,
            )
            .err()
            .expect("N+1 cannot persist before N finalizes")
            .to_string()
            .contains("non-current sequence")
        );
        assert!(!series_acquired_address_frame_path_v2(&directory, 4).exists());
        let mut preauthored = first.clone();
        preauthored.sequence = 4;
        preauthored.recipe = future_recipe.clone();
        preauthored.frame_sha256 = acquired_address_frame_digest_v2(&preauthored).unwrap();
        create_series_canonical_json_v1(
            &series_acquired_address_frame_path_v2(&directory, 4),
            &preauthored,
            "test-only future acquired frame",
        )
        .unwrap();
        assert!(
            authenticate_series_acquired_address_frame_history_v2(
                &directory,
                &[recipe.clone(), future_recipe],
                &[],
                3,
                &"11".repeat(32),
                &ledger(),
            )
            .err()
            .expect("preauthored future frame must refuse")
            .to_string()
            .contains("preauthored a future")
        );
        let mut substituted = first.clone();
        substituted.recipe.runtime_logical_accounts[0] = key(99).to_string();
        substituted.frame_sha256 = acquired_address_frame_digest_v2(&substituted).unwrap();
        let mut substituted_bytes = serde_json::to_vec(&substituted).unwrap();
        substituted_bytes.push(b'\n');
        fs::write(
            series_acquired_address_frame_path_v2(&directory, 3),
            substituted_bytes,
        )
        .unwrap();
        assert!(
            authenticate_series_acquired_address_frame_history_v2(
                &directory,
                std::slice::from_ref(&recipe),
                &[],
                3,
                &"11".repeat(32),
                &ledger(),
            )
            .err()
            .expect("restart must refuse a caller-substituted address frame")
            .to_string()
            .contains("address recipe")
        );
        fs::remove_file(series_acquired_address_frame_path_v2(&directory, 3))
            .expect("remove acquired frame");
        fs::remove_file(series_acquired_address_frame_path_v2(&directory, 4))
            .expect("remove test-only future frame");
        fs::remove_dir(&directory).expect("remove test directory");
    }

    #[test]
    fn command_refuses_every_caller_selected_action_surface() {
        for forbidden in [
            "--action",
            "--retire",
            "--close",
            "--selected-report",
            "--direct-terminal-completion",
        ] {
            let error = parse_series_terminal_arguments_v1(vec![forbidden.to_owned()])
                .expect_err("caller-selected action surface must refuse");
            assert!(
                error
                    .to_string()
                    .contains("unknown Series terminal campaign argument"),
                "unexpected refusal for {forbidden}: {error}"
            );
        }
        let text = usage();
        assert!(text.contains("lifecycle planner choose the act"));
        assert!(text.contains("Prepared is the first durable action-journal boundary"));
        assert!(text.contains("future durable frame is refused"));
        assert!(!text.contains("--action"));
        assert!(!text.contains("--retire"));
        assert!(!text.contains("--close"));
        assert!(!text.contains("Direct"));
    }

    #[test]
    fn caller_authored_privileges_are_not_an_acquisition_language() {
        let legacy_logical = serde_json::json!([{
            "address": key(1).to_string(),
            "signer": false,
            "writable": true,
        }]);
        assert!(serde_json::from_value::<Vec<String>>(legacy_logical).is_err());
        let legacy_record = serde_json::json!({
            "raw": key(2).to_string(),
            "staging": key(3).to_string(),
            "signer": false,
            "writable": false,
        });
        assert!(serde_json::from_value::<SeriesFinalizedRecordAddressesV2>(legacy_record).is_err());
    }

    #[test]
    fn preflight_refuses_non_payer_signer_before_any_key_read() {
        let payer = key(13);
        let mut instruction = selected(SeriesActionV3::Close).instruction;
        authenticate_permissionless_series_signers_v1(&instruction, payer)
            .expect("signer-free selected frame");
        instruction
            .accounts
            .push(AccountMeta::new_readonly(payer, true));
        authenticate_permissionless_series_signers_v1(&instruction, payer)
            .expect("payer may be the sole instruction signer");
        instruction
            .accounts
            .push(AccountMeta::new_readonly(key(14), true));
        let error = authenticate_permissionless_series_signers_v1(&instruction, payer)
            .expect_err("additional signer must refuse");
        assert!(
            error.to_string().contains("additional signer not supplied"),
            "unexpected additional-signer refusal: {error}"
        );
    }

    #[test]
    fn selected_occurrence_roles_require_expire_only_permit() {
        let occurrence_roles = |permit| SeriesPhysicalRoleKeysV1 {
            root: key(10),
            ticket: Some(key(11)),
            rent_credit: None,
            parent_market: key(9),
            parent_market_generation: 1,
            occurrence_market: Some(key(31)),
            occurrence_market_generation: Some(1),
            occurrence_permit: permit,
        };
        for action in [SeriesActionV3::Prepare, SeriesActionV3::Consume] {
            let error = validate_mechanism_and_roles_v1(
                SeriesJournalActionV1::from_kernel(action),
                SeriesPhysicalMechanismV1::Occurrence,
                occurrence_roles(Some(key(32))),
            )
            .expect_err("pre-Found occurrence permit must refuse");
            assert!(
                error.to_string().contains("exact V5 terminal mechanism"),
                "unexpected {action:?} permit refusal: {error}"
            );
        }

        let error = validate_mechanism_and_roles_v1(
            SeriesJournalActionV1::Expire,
            SeriesPhysicalMechanismV1::Occurrence,
            occurrence_roles(None),
        )
        .expect_err("Expire without authenticated permit must refuse");
        assert!(
            error.to_string().contains("exact V5 terminal mechanism"),
            "unexpected Expire permit refusal: {error}"
        );
    }

    #[test]
    fn retirement_snapshot_binds_every_role_payer_table_and_map_key() {
        let role_addresses = SERIES_RETIREMENT_ROLES_V1
            .iter()
            .enumerate()
            .map(|(index, role)| {
                (
                    (*role).to_owned(),
                    key(u8::try_from(index + 1).expect("bounded role index")).to_string(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let route = SeriesMarketRetirementRouteV1 {
            ordinal: 0,
            role_addresses: role_addresses.clone(),
            snapshot: PathBuf::from("/tmp/series-retirement-snapshot.json"),
            campaign: PathBuf::from("/tmp/series-retirement-campaign.json"),
            journal_dir: PathBuf::from("/tmp/series-retirement-journal"),
            completion: PathBuf::from("/tmp/series-retirement-completion.json"),
        };
        let payer = key(100);
        let lookup_table = key(101);
        let mut accounts = role_addresses
            .values()
            .map(|address| {
                let key = parse_pubkey(address, "test retirement role").expect("role key");
                (address.clone(), durable_absent_account_v1(key))
            })
            .collect::<BTreeMap<_, _>>();
        accounts.insert(
            payer.to_string(),
            durable_present_account_v1(SeriesObservedAccountV1 {
                key: payer,
                owner: solana_sdk_ids::system_program::ID,
                lamports: 1,
                executable: false,
                data: Vec::new(),
            })
            .expect("payer"),
        );
        accounts.insert(
            lookup_table.to_string(),
            durable_present_account_v1(SeriesObservedAccountV1 {
                key: lookup_table,
                owner: lookup_table_program::ID,
                lamports: 1,
                executable: false,
                data: vec![1],
            })
            .expect("lookup table"),
        );
        let ledger = ledger();
        let mut snapshot = DurableSeriesMarketRetirementSnapshotV1 {
            schema: SERIES_MARKET_RETIREMENT_SNAPSHOT_SCHEMA_V1.into(),
            campaign_sha256: "11".repeat(32),
            ledger_identity_sha256: ledger.identity_sha256.clone(),
            ordinal: 0,
            payer: payer.to_string(),
            lookup_table: lookup_table.to_string(),
            observation_slot: 41,
            observation_unix_timestamp: 1,
            role_addresses,
            accounts,
            snapshot_sha256: String::new(),
        };
        snapshot.snapshot_sha256 =
            series_retirement_snapshot_digest_v1(&snapshot).expect("snapshot digest");
        authenticate_durable_series_retirement_snapshot_v1(
            &snapshot,
            &route,
            &snapshot.campaign_sha256,
            &ledger,
            payer,
            lookup_table,
        )
        .expect("complete retirement snapshot");

        let mut changed_key = snapshot.clone();
        let first_address = changed_key
            .accounts
            .keys()
            .next()
            .expect("first account")
            .clone();
        let first = changed_key
            .accounts
            .remove(&first_address)
            .expect("first durable account");
        changed_key.accounts.insert(key(110).to_string(), first);
        changed_key.snapshot_sha256 =
            series_retirement_snapshot_digest_v1(&changed_key).expect("changed digest");
        let error = authenticate_durable_series_retirement_snapshot_v1(
            &changed_key,
            &route,
            &changed_key.campaign_sha256,
            &ledger,
            payer,
            lookup_table,
        )
        .expect_err("map-key substitution must refuse");
        assert!(
            error
                .to_string()
                .contains("map key changed its account identity"),
            "unexpected map-key refusal: {error}"
        );

        let mut changed_payer = snapshot;
        changed_payer.payer = key(111).to_string();
        changed_payer.snapshot_sha256 =
            series_retirement_snapshot_digest_v1(&changed_payer).expect("changed digest");
        let error = authenticate_durable_series_retirement_snapshot_v1(
            &changed_payer,
            &route,
            &changed_payer.campaign_sha256,
            &ledger,
            payer,
            lookup_table,
        )
        .expect_err("payer substitution must refuse");
        assert!(
            error
                .to_string()
                .contains("changed identity, observation, or routing"),
            "unexpected payer refusal: {error}"
        );
    }
}
