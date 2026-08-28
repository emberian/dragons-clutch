//! Devnet-only keeper for one slot-seeded relayed observation record.
//!
//! The keeper is deliberately separate from `txn`'s append/seal surface.  It
//! authenticates the complete creation graph at one finalized slot, persists
//! an unsigned plan, and only then may load the explicitly configured fee
//! payer.  A crash after submission is safe: the next run accepts only the
//! exact canonical `Collecting` record that the saved plan predicted.

use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs::OpenOptions, io::Write};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase, Readiness};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1,
};
use dclutch_relay_contract::{
    RELAYED_ADAPTER_CONFIG_BYTES, RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
    RELAYED_ATTESTATION_HEAD_BYTES, RELAYED_FAMILY_RELEASE_ID_V1,
    RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1, RELAYED_SEAL_BYTES, RELAYER_KEY_SET_BYTES,
    RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1, SOLANA_DEVNET_GENESIS_HASH_V1,
    SOLANA_MAINNET_GENESIS_HASH_V1,
    frame::{
        RelayAccountNameV1, RelayAccountPrivilegeV1, RelayFrameKindV1, relay_frame_roles_v1,
        validate_relay_frame_v1,
    },
    instruction::CreateRecordInstructionV1,
    record::{
        RelayedObservationRecordViewV1, RelayedRecordPhaseV1, relayed_observation_record_bytes_v1,
    },
    release::{RelayedAdapterConfigV1, RelayerKeySetV1},
    release::{SET_DIGEST_SEED_PREIMAGE_BYTES, encode_set_digest_seed_preimage_v1},
    wire::{AttestationMessageV1, ObservationSetSealV1},
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_source_contract::{
    PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_SCHEMA_ID_V1, ProviderReleaseV1,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_MATERIAL_V3_BYTES, SOURCE_SPEC_BYTES,
    SOURCE_SPEC_SCHEMA_ID_V1, SourceAccessProfile, SourceMaterialV3, SourceSpecV1,
    WINDOW_SPEC_BYTES, WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_message::{AddressLookupTableAccount, VersionedMessage, legacy, v0};
use solana_rent::Rent;

use crate::chain::{
    CLOCK_SYSVAR_ID, COMPUTE_BUDGET_PROGRAM_ID, COMPUTE_BUDGET_SET_UNIT_LIMIT_TAG,
    COMPUTE_BUDGET_SET_UNIT_PRICE_TAG, RENT_SYSVAR_ID,
};
use crate::config::{AccountSetConfig, Config, SubmitConfig};
use crate::error::{RelayerError, Result};
use crate::id32::{ID_BYTES, base58, parse_id32, to_hex};
use crate::keys::AttestationSigner;
use crate::rpc::{BatchRead, ObservedAccount, RpcClient, base64_encode};
use crate::submit::require_submission_admitted;
use crate::txn::{
    SOLANA_PACKET_DATA_BYTES, message_bytes, require_packet_fit, serialize_transaction,
    sign_transaction,
};

const PLAN_FORMAT_V1: &str = "dclutch-relay-create-record-plan-v1";
const RECEIPT_FORMAT_V1: &str = "dclutch-relay-create-record-receipt-v1";
const PRESTATE_DOMAIN_V1: &[u8] = b"dclutch/relay-create-record-prestate/v1";
const SYSTEM_PROGRAM_ID: [u8; ID_BYTES] = [0; ID_BYTES];
const SYSVAR_PROGRAM_ID_BASE58: &str = "Sysvar1111111111111111111111111111111111111";
const NATIVE_LOADER_ID_BASE58: &str = "NativeLoader1111111111111111111111111111111";
const MICRO_LAMPORTS_PER_LAMPORT: u128 = 1_000_000;
const POSTSTATE_POLLS: usize = 300;

/// Inputs to one keeper invocation.
pub struct CreateRecordKeeperRequest<'a> {
    /// Fully resolved relayer configuration.
    pub config: &'a Config,
    /// One dry-run artifact directory.
    pub slot_dir: &'a Path,
    /// Explicit fee-payer/worker public key.  Read-only mode opens no key.
    pub worker: [u8; ID_BYTES],
    /// Whether to load the configured fee payer and submit after persisting.
    pub execute: bool,
    /// Home directory used only for the existing safe-key-path policy.
    pub home: Option<&'a Path>,
}

/// Result reported by the keeper command.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRecordKeeperReportV1 {
    /// Persisted immutable plan path.
    pub plan_path: PathBuf,
    /// Whether the exact record was already finalized.
    pub already_complete: bool,
    /// Submitted transaction signature, when execution occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Persisted receipt path, after an exact finalized poststate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_path: Option<PathBuf>,
}

/// Exact action remaining for an append/seal artifact after finalized resume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordResumeV1 {
    /// Append the artifact starting at this first unpersisted set index.
    AppendFrom(u16),
    /// The complete artifact is already present in an exact sealed record.
    Complete,
}

/// Authenticate an existing finalized record against one exact signed artifact.
///
/// Every persisted prefix body and the running digest must equal the artifact;
/// a caller can therefore resume after a crash without replaying position zero
/// or silently accepting another relayer's divergent prefix.
#[allow(clippy::too_many_arguments)]
pub fn inspect_record_resume_v1(
    relay_program: [u8; ID_BYTES],
    record_address: [u8; ID_BYTES],
    market: [u8; ID_BYTES],
    generation: u64,
    account_set_id: [u8; ID_BYTES],
    observed_slot: u64,
    record: &ObservedAccount,
    attestation_messages: &[&[u8]],
    seal_bytes: &[u8],
) -> Result<RecordResumeV1> {
    let set_count = u16::try_from(attestation_messages.len())
        .map_err(|_| RelayerError::config("artifact attestation count does not fit u16"))?;
    let expected_record = crate::txn::derive_record_address(
        relay_program,
        market,
        generation,
        account_set_id,
        observed_slot,
    )
    .0;
    let width = relayed_observation_record_bytes_v1(set_count)
        .map_err(|error| RelayerError::wire("resume record width", error))?;
    if record_address != expected_record
        || record.owner != relay_program
        || record.executable
        || record.data_len != width as u64
        || record.data.len() != width
    {
        return Err(RelayerError::config(
            "finalized resume record address/owner/width refused",
        ));
    }
    let view = RelayedObservationRecordViewV1::decode(&record.data)
        .map_err(|error| RelayerError::wire("finalized resume record", error))?;
    let filled = view
        .filled_count()
        .map_err(|error| RelayerError::wire("resume filled count", error))?;
    if view
        .market()
        .map_err(|error| RelayerError::wire("resume Market", error))?
        != market
        || view
            .generation()
            .map_err(|error| RelayerError::wire("resume generation", error))?
            != generation
        || view
            .account_set_id()
            .map_err(|error| RelayerError::wire("resume account set", error))?
            != account_set_id
        || view
            .observed_cluster_id()
            .map_err(|error| RelayerError::wire("resume observed cluster", error))?
            != SOLANA_MAINNET_GENESIS_HASH_V1
        || view
            .observed_slot()
            .map_err(|error| RelayerError::wire("resume observed slot", error))?
            != observed_slot
        || view
            .set_count()
            .map_err(|error| RelayerError::wire("resume set count", error))?
            != set_count
        || filled > set_count
    {
        return Err(RelayerError::config(
            "finalized resume record binding/geometry refused",
        ));
    }

    let seal = ObservationSetSealV1::decode(seal_bytes)
        .map_err(|error| RelayerError::wire("resume artifact seal", error))?;
    if seal.observed_cluster_id() != SOLANA_MAINNET_GENESIS_HASH_V1
        || seal.relay_family_id() != RELAYED_FAMILY_RELEASE_ID_V1
        || seal.account_set_id() != account_set_id
        || seal.observed_slot() != observed_slot
        || seal.set_count() != set_count
    {
        return Err(RelayerError::config(
            "resume artifact seal binding/geometry refused",
        ));
    }

    let mut fold = crate::derive::SetDigestFold::seed(account_set_id, observed_slot)?;
    let mut prefix_digest = fold.digest();
    for (index, bytes) in attestation_messages.iter().enumerate() {
        let set_index = u16::try_from(index)
            .map_err(|_| RelayerError::config("resume set index does not fit u16"))?;
        let message = AttestationMessageV1::decode(bytes)
            .map_err(|error| RelayerError::wire("resume artifact attestation", error))?;
        if message.observed_cluster_id() != SOLANA_MAINNET_GENESIS_HASH_V1
            || message.relay_family_id() != RELAYED_FAMILY_RELEASE_ID_V1
            || message.account_set_id() != account_set_id
            || message.observed_slot() != observed_slot
            || message.set_index() != set_index
            || message.set_count() != set_count
        {
            return Err(RelayerError::config(
                "resume artifact attestation binding/order refused",
            ));
        }
        let body_width = message.body().encoded_len();
        let body_bytes = bytes
            .get(RELAYED_ATTESTATION_HEAD_BYTES..)
            .filter(|body| body.len() == body_width)
            .ok_or_else(|| RelayerError::config("resume attestation body width refused"))?;
        fold.absorb(body_bytes);
        if set_index < filled {
            let persisted = view
                .observation(set_index)
                .map_err(|error| RelayerError::wire("persisted resume observation", error))?;
            if persisted != message.body() {
                return Err(RelayerError::config(format!(
                    "finalized record position {set_index} differs from the signed artifact"
                )));
            }
            prefix_digest = fold.digest();
        }
    }
    if seal.set_digest() != fold.digest() {
        return Err(RelayerError::config(
            "resume artifact seal does not match its complete attestation fold",
        ));
    }

    let persisted_digest = view
        .set_digest()
        .map_err(|error| RelayerError::wire("resume persisted set digest", error))?;
    match view
        .phase()
        .map_err(|error| RelayerError::wire("resume record phase", error))?
    {
        RelayedRecordPhaseV1::Collecting if persisted_digest == prefix_digest => {
            Ok(RecordResumeV1::AppendFrom(filled))
        }
        RelayedRecordPhaseV1::Sealed
            if filled == set_count && persisted_digest == seal.set_digest() =>
        {
            Ok(RecordResumeV1::Complete)
        }
        RelayedRecordPhaseV1::Collecting | RelayedRecordPhaseV1::Sealed => {
            Err(RelayerError::config(
                "finalized record digest/phase differs from the exact artifact prefix",
            ))
        }
        RelayedRecordPhaseV1::Consumed | RelayedRecordPhaseV1::Retired => Err(
            RelayerError::config("append/seal resume refuses a terminal observation record"),
        ),
    }
}

#[derive(Clone, Debug)]
struct ArtifactRouteV1 {
    set_name: String,
    account_set_id: [u8; ID_BYTES],
    observed_slot: u64,
    set_count: u16,
    decoding_rules_id: [u8; ID_BYTES],
    signer: [u8; ID_BYTES],
}

#[derive(Clone, Copy, Debug)]
struct RecordPairV1 {
    raw: [u8; ID_BYTES],
    staging: [u8; ID_BYTES],
}

#[derive(Clone, Debug)]
struct FrameKeysV1 {
    worker: [u8; ID_BYTES],
    market: [u8; ID_BYTES],
    core: [u8; ID_BYTES],
    activation: [u8; ID_BYTES],
    record: [u8; ID_BYTES],
    material: RecordPairV1,
    spec: RecordPairV1,
    provider: RecordPairV1,
    window: RecordPairV1,
    key_set: RecordPairV1,
    adapter: RecordPairV1,
    beneficiary: [u8; ID_BYTES],
}

#[derive(Clone, Debug)]
struct AuthenticatedCreateV1 {
    snapshot: FrameSnapshotV1,
    keys: FrameKeysV1,
    generation: u64,
    source_material_id: [u8; ID_BYTES],
    source_spec_id: [u8; ID_BYTES],
    provider_release_id: [u8; ID_BYTES],
    relayer_key_set_id: [u8; ID_BYTES],
    account_set_id: [u8; ID_BYTES],
    observed_slot: u64,
    set_count: u16,
    seal_threshold: u8,
    pda_bump: u8,
    record_width: usize,
    record_rent_minimum: u64,
    record_lamports: u64,
    worker_lamports: u64,
    already_complete: bool,
}

#[derive(Clone, Debug)]
struct FrameSnapshotV1 {
    slot: u64,
    keys: Vec<[u8; ID_BYTES]>,
    accounts: Vec<Option<ObservedAccount>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateRecordPlanV1 {
    format: &'static str,
    status: &'static str,
    finalized_snapshot_slot: u64,
    submit_cluster_genesis: String,
    market: String,
    generation: u64,
    account_set_id: String,
    observed_slot: u64,
    record: String,
    prestate_sha256: String,
    instruction: PlannedInstructionV1,
    funding: FundingPlanV1,
    packet_bound: PacketBoundV1,
    expected_poststate: ExpectedPoststateV1,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlannedInstructionV1 {
    program_id: String,
    accounts: Vec<PlannedAccountMetaV1>,
    data_base64: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlannedAccountMetaV1 {
    role: &'static str,
    address: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FundingPlanV1 {
    worker_balance_lamports: u64,
    existing_record_lamports: u64,
    record_rent_minimum_lamports: u64,
    record_top_up_lamports: u64,
    priority_fee_ceiling_lamports: u64,
    minimum_before_base_fee_lamports: u64,
    base_fee: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PacketBoundV1 {
    selected_message_version: &'static str,
    legacy_packet_bytes: usize,
    v0_packet_bytes: usize,
    maximum_packet_bytes: usize,
    lookup_table_addresses: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedPoststateV1 {
    owner: String,
    phase: &'static str,
    set_count: u16,
    seal_threshold: u8,
    filled_count: u16,
    seal_count: u8,
    minimum_lamports: u64,
    created_unix_seconds: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateRecordReceiptV1 {
    format: &'static str,
    plan_prestate_sha256: String,
    record: String,
    transaction_signature: Option<String>,
    finalized_slot: u64,
    record_lamports: u64,
    record_rent_minimum_lamports: u64,
    record_top_up_lamports: u64,
    transaction_fee_lamports: u64,
    total_worker_debit_lamports: u64,
}

/// Authenticate, persist, and optionally execute exactly one create-record plan.
pub async fn run_create_record_keeper(
    request: CreateRecordKeeperRequest<'_>,
) -> Result<CreateRecordKeeperReportV1> {
    let artifact = read_artifact(request.slot_dir, request.config)?;
    let submit = require_devnet_submit(request.config)?;
    let rpc = RpcClient::new(&submit.endpoint, request.config.request_timeout, None)?
        .logging_to(&request.config.output_dir)?;
    rpc.require_expected_genesis(SOLANA_DEVNET_GENESIS_HASH_V1)
        .await?;

    let authenticated =
        discover_and_authenticate(&rpc, submit, &artifact, request.config, request.worker).await?;
    let instruction = build_create_instruction(submit.relay_program_id, &authenticated)?;
    let plan = build_plan(submit, &authenticated, &instruction)?;
    let plan_path = persist_plan(request.slot_dir, &plan)?;

    if authenticated.already_complete {
        let receipt = receipt_for(&plan, &authenticated, None, authenticated.snapshot.slot, 0)?;
        let receipt_path = persist_receipt(request.slot_dir, &receipt)?;
        return Ok(CreateRecordKeeperReportV1 {
            plan_path,
            already_complete: true,
            signature: None,
            receipt_path: Some(receipt_path),
        });
    }
    if !request.execute {
        return Ok(CreateRecordKeeperReportV1 {
            plan_path,
            already_complete: false,
            signature: None,
            receipt_path: None,
        });
    }

    require_submission_admitted(submit)?;
    rpc.require_expected_genesis(SOLANA_DEVNET_GENESIS_HASH_V1)
        .await?;
    let fee_payer_path = request
        .config
        .fee_payer_keypair_path
        .as_ref()
        .ok_or_else(|| {
            RelayerError::MissingCapability(
                "create-record --execute needs keys.fee_payer_keypair_path".to_owned(),
            )
        })?;
    let fee_payer = AttestationSigner::load(fee_payer_path, request.home)?;
    if fee_payer.public_key() != request.worker {
        return Err(RelayerError::config(format!(
            "the configured fee payer is {}, not the explicit --worker {} used by the persisted plan",
            fee_payer.public_key_base58(),
            base58(&request.worker)
        )));
    }

    // Reauthenticate after the key is loaded.  A changed prestate never gets a
    // signature under a plan produced before that change.
    let refreshed =
        discover_and_authenticate(&rpc, submit, &artifact, request.config, request.worker).await?;
    if refreshed.already_complete {
        let receipt = receipt_for(&plan, &refreshed, None, refreshed.snapshot.slot, 0)?;
        let receipt_path = persist_receipt(request.slot_dir, &receipt)?;
        return Ok(CreateRecordKeeperReportV1 {
            plan_path,
            already_complete: true,
            signature: None,
            receipt_path: Some(receipt_path),
        });
    }
    if prestate_digest(&refreshed.snapshot) != prestate_digest(&authenticated.snapshot) {
        return Err(RelayerError::config(
            "create-record prestate changed after the unsigned plan was persisted; rerun to produce a new plan",
        ));
    }

    rpc.require_expected_genesis(SOLANA_DEVNET_GENESIS_HASH_V1)
        .await?;
    let (blockhash_text, _) = rpc.get_latest_blockhash().await?;
    let blockhash = parse_id32("devnet recent blockhash", &blockhash_text)?;
    let built = compile_selected_message(submit, request.worker, &instruction, blockhash)?;
    let unsigned_message = message_bytes(&built);
    let fee = rpc
        .get_fee_for_message(&base64_encode(&unsigned_message))
        .await?;
    let top_up = refreshed
        .record_rent_minimum
        .saturating_sub(refreshed.record_lamports);
    let total_debit = top_up
        .checked_add(fee)
        .ok_or_else(|| RelayerError::config("record rent plus transaction fee overflowed"))?;
    if refreshed.worker_lamports < total_debit {
        return Err(RelayerError::config(format!(
            "worker has {} lamports but this finalized prestate needs {top_up} record-rent lamports + {fee} transaction-fee lamports = {total_debit}",
            refreshed.worker_lamports
        )));
    }

    rpc.require_expected_genesis(SOLANA_DEVNET_GENESIS_HASH_V1)
        .await?;
    let signature = fee_payer.sign(&unsigned_message);
    let transaction = sign_transaction(built, signature);
    let wire = serialize_transaction(&transaction)?;
    let routed = submit
        .address_lookup_table
        .as_ref()
        .map_or(0, |table| table.addresses.len());
    require_packet_fit("create observation record", &wire, routed)?;
    rpc.require_expected_genesis(SOLANA_DEVNET_GENESIS_HASH_V1)
        .await?;
    let submitted = rpc.send_transaction(&base64_encode(&wire)).await?;
    let (finalized_slot, finalized_lamports) = await_exact_poststate(
        &rpc,
        submit.relay_program_id,
        &refreshed,
        refreshed.snapshot.slot,
    )
    .await?;
    let receipt = receipt_for(
        &plan,
        &refreshed,
        Some(submitted.clone()),
        finalized_slot,
        fee,
    )?;
    let mut receipt = receipt;
    receipt.record_lamports = finalized_lamports;
    let receipt_path = persist_receipt(request.slot_dir, &receipt)?;
    Ok(CreateRecordKeeperReportV1 {
        plan_path,
        already_complete: false,
        signature: Some(submitted),
        receipt_path: Some(receipt_path),
    })
}

fn read_artifact(slot_dir: &Path, config: &Config) -> Result<ArtifactRouteV1> {
    let path = slot_dir.join("manifest.json");
    let text = std::fs::read_to_string(&path).map_err(|source| RelayerError::io(&path, source))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|source| RelayerError::Serialization(source.to_string()))?;
    if value.get("artifact_schema").and_then(Value::as_str) != Some("dclutch.relayer.dry-run.v1") {
        return Err(RelayerError::config(format!(
            "{} is not a dclutch.relayer.dry-run.v1 artifact",
            path.display()
        )));
    }
    if value
        .get("rehearsal_twin")
        .is_some_and(|entry| !entry.is_null())
        || config.rehearsal_attested_cluster_id.is_some()
    {
        return Err(RelayerError::config(
            "a rehearsal-twin artifact can never seed a record on public devnet",
        ));
    }
    let set_name = value
        .get("set_name")
        .and_then(Value::as_str)
        .ok_or_else(|| RelayerError::config("artifact manifest has no set_name"))?
        .to_owned();
    let account_set_id = parse_id32(
        "manifest.account_set_id_hex",
        value
            .get("account_set_id_hex")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )?;
    let decoding_rules_id = parse_id32(
        "manifest.decoding_rules_id_hex",
        value
            .get("decoding_rules_id_hex")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )?;
    let observed_cluster = parse_id32(
        "manifest.observed_cluster_id_hex",
        value
            .get("observed_cluster_id_hex")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )?;
    let relay_family = parse_id32(
        "manifest.relay_family_id_hex",
        value
            .get("relay_family_id_hex")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )?;
    let observed_slot = value
        .get("observed_slot")
        .and_then(Value::as_u64)
        .filter(|slot| *slot != 0)
        .ok_or_else(|| RelayerError::config("artifact observed_slot must be positive"))?;
    let set_count = u16::try_from(
        value
            .get("set_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| RelayerError::config("artifact manifest has no set_count"))?,
    )
    .map_err(|_| RelayerError::config("artifact set_count does not fit a u16"))?;
    let position_count = value
        .get("positions")
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| RelayerError::config("artifact manifest has no positions"))?;
    if observed_cluster != SOLANA_MAINNET_GENESIS_HASH_V1
        || relay_family != RELAYED_FAMILY_RELEASE_ID_V1
        || usize::from(set_count) != position_count
    {
        return Err(RelayerError::config(
            "artifact cluster/family/set geometry is not the canonical relayed-mainnet release",
        ));
    }
    let configured = configured_set(config, &set_name)?;
    if configured.account_set_id != account_set_id
        || configured.decoding_rules_id != decoding_rules_id
        || configured.relay_family_id != relay_family
        || configured.set_count()? != set_count
    {
        return Err(RelayerError::config(format!(
            "artifact set {set_name:?} no longer matches the config's derived release identities"
        )));
    }
    let signer = parse_id32(
        "manifest.attestation_signer_pubkey_hex",
        value
            .get("attestation_signer_pubkey_hex")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )?;
    let manifest_set_digest = parse_id32(
        "manifest.set_digest_hex",
        value
            .get("set_digest_hex")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )?;
    let positions = value
        .get("positions")
        .and_then(Value::as_array)
        .ok_or_else(|| RelayerError::config("artifact manifest has no positions"))?;
    let mut seed = [0u8; SET_DIGEST_SEED_PREIMAGE_BYTES];
    encode_set_digest_seed_preimage_v1(&mut seed, account_set_id, observed_slot)
        .map_err(|error| RelayerError::wire("artifact set-digest seed", error))?;
    let mut running: [u8; ID_BYTES] = Sha256::digest(seed).into();
    for (expected_index, (position, configured_position)) in
        positions.iter().zip(&configured.positions).enumerate()
    {
        let set_index = u16::try_from(expected_index)
            .map_err(|_| RelayerError::config("artifact position index does not fit u16"))?;
        if position.get("set_index").and_then(Value::as_u64) != Some(u64::from(set_index)) {
            return Err(RelayerError::config(
                "artifact positions are not in exact canonical set order",
            ));
        }
        let message_file = format!("attestation.{set_index}.bin");
        let signature_file = format!("attestation.{set_index}.sig");
        if position.get("message_file").and_then(Value::as_str) != Some(&message_file)
            || position.get("signature_file").and_then(Value::as_str) != Some(&signature_file)
        {
            return Err(RelayerError::config(
                "artifact position names a non-canonical or traversing payload path",
            ));
        }
        let message_path = slot_dir.join(&message_file);
        let message = std::fs::read(&message_path)
            .map_err(|source| RelayerError::io(&message_path, source))?;
        let signature_path = slot_dir.join(&signature_file);
        let signature_bytes = std::fs::read(&signature_path)
            .map_err(|source| RelayerError::io(&signature_path, source))?;
        let signature: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| RelayerError::config("artifact signature is not exactly 64 bytes"))?;
        if !crate::keys::verify_detached(&signer, &message, &signature) {
            return Err(RelayerError::config(
                "artifact attestation signature does not verify",
            ));
        }
        let decoded = AttestationMessageV1::decode(&message)
            .map_err(|error| RelayerError::wire("artifact attestation", error))?;
        if decoded.observed_cluster_id() != SOLANA_MAINNET_GENESIS_HASH_V1
            || decoded.relay_family_id() != RELAYED_FAMILY_RELEASE_ID_V1
            || decoded.decoding_rules_id() != decoding_rules_id
            || decoded.account_set_id() != account_set_id
            || decoded.observed_slot() != observed_slot
            || decoded.set_index() != set_index
            || decoded.set_count() != set_count
            || !configured_position.admits_data_len(decoded.body().data_len())
            || decoded
                .body()
                .require_pinned_position(
                    configured_position.key,
                    configured_position.expected_owner,
                    configured_position.inline_len,
                )
                .is_err()
        {
            return Err(RelayerError::config(
                "artifact attestation differs from its config/set/slot/position authority",
            ));
        }
        let body_width = decoded.body().encoded_len();
        let body = message
            .get(RELAYED_ATTESTATION_HEAD_BYTES..)
            .filter(|body| body.len() == body_width)
            .ok_or_else(|| RelayerError::config("artifact attestation body width refused"))?;
        let mut fold = Sha256::new();
        fold.update(running);
        fold.update(body);
        running = fold.finalize().into();
    }
    let seal_path = slot_dir.join("seal.bin");
    let seal_bytes =
        std::fs::read(&seal_path).map_err(|source| RelayerError::io(&seal_path, source))?;
    if seal_bytes.len() != RELAYED_SEAL_BYTES {
        return Err(RelayerError::config("artifact seal has the wrong width"));
    }
    let seal_signature_path = slot_dir.join("seal.sig");
    let seal_signature_bytes = std::fs::read(&seal_signature_path)
        .map_err(|source| RelayerError::io(&seal_signature_path, source))?;
    let seal_signature: [u8; 64] = seal_signature_bytes
        .try_into()
        .map_err(|_| RelayerError::config("artifact seal signature is not 64 bytes"))?;
    if !crate::keys::verify_detached(&signer, &seal_bytes, &seal_signature) {
        return Err(RelayerError::config(
            "artifact seal signature does not verify",
        ));
    }
    let seal = ObservationSetSealV1::decode(&seal_bytes)
        .map_err(|error| RelayerError::wire("artifact seal", error))?;
    if seal.observed_cluster_id() != SOLANA_MAINNET_GENESIS_HASH_V1
        || seal.relay_family_id() != RELAYED_FAMILY_RELEASE_ID_V1
        || seal.account_set_id() != account_set_id
        || seal.observed_slot() != observed_slot
        || seal.set_count() != set_count
        || seal.set_digest() != running
        || manifest_set_digest != running
    {
        return Err(RelayerError::config(
            "artifact seal differs from the authenticated ordered attestation fold",
        ));
    }
    Ok(ArtifactRouteV1 {
        set_name,
        account_set_id,
        observed_slot,
        set_count,
        decoding_rules_id,
        signer,
    })
}

fn configured_set<'a>(config: &'a Config, name: &str) -> Result<&'a AccountSetConfig> {
    config
        .account_sets
        .iter()
        .find(|set| set.name == name)
        .ok_or_else(|| RelayerError::config(format!("config watches no set named {name:?}")))
}

fn require_devnet_submit(config: &Config) -> Result<&SubmitConfig> {
    let submit = config.submit.as_ref().ok_or_else(|| {
        RelayerError::MissingCapability(
            "create-record needs a [submit] table naming the devnet Market".to_owned(),
        )
    })?;
    if submit.expected_genesis_hash != SOLANA_DEVNET_GENESIS_HASH_V1 {
        return Err(RelayerError::config(format!(
            "create-record accepts only exact Solana devnet genesis {}; the config named {}",
            base58(&SOLANA_DEVNET_GENESIS_HASH_V1),
            base58(&submit.expected_genesis_hash)
        )));
    }
    Ok(submit)
}

async fn discover_and_authenticate(
    rpc: &RpcClient,
    submit: &SubmitConfig,
    artifact: &ArtifactRouteV1,
    config: &Config,
    worker: [u8; ID_BYTES],
) -> Result<AuthenticatedCreateV1> {
    let market_read = rpc
        .get_multiple_accounts(&[submit.market], u16::MAX, None)
        .await?;
    let market_account = required_batch(&market_read, 0, "Core Market")?;
    let market = CoreState::decode(&market_account.data)
        .map_err(|error| RelayerError::config(format!("Core Market refused: {error:?}")))?;
    authenticate_market(submit, market_account, market)?;
    let core = market_account.owner;
    let registry = market.identity.registry_program.to_bytes();
    let release_set = market.identity.selected_release_set.to_bytes();
    let source_material_id = market.identity.resolution_policy.to_bytes();
    let beneficiary = market.rent_beneficiary.to_bytes();
    let activation = derive_pda(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_slice()],
        registry,
    );
    let material = record_pair(
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        source_material_id,
    );

    let first_keys = [
        core,
        activation,
        RENT_SYSVAR_ID,
        CLOCK_SYSVAR_ID,
        SYSTEM_PROGRAM_ID,
        beneficiary,
        material.raw,
        material.staging,
    ];
    let first = rpc
        .get_multiple_accounts(&first_keys, u16::MAX, Some(market_read.slot))
        .await?;
    let rent = decode_rent(required_batch(&first, 2, "Rent sysvar")?)?;
    let material_account = required_batch(&first, 6, "SourceMaterialV3")?;
    authenticate_record_pair(
        registry,
        material,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        source_material_id,
        material_account,
        first.accounts.get(7).and_then(Option::as_ref),
        &rent,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material_value = SourceMaterialV3::decode(&material_account.data)
        .map_err(|error| RelayerError::config(format!("SourceMaterialV3 refused: {error:?}")))?;
    let source_spec_id = material_value.primary_source_spec().to_bytes();
    let window_id = material_value.window_spec().to_bytes();
    let spec = record_pair(registry, SOURCE_SPEC_SCHEMA_ID_V1, source_spec_id);
    let window = record_pair(registry, WINDOW_SPEC_SCHEMA_ID_V1, window_id);

    let second_keys = [spec.raw, spec.staging, window.raw, window.staging];
    let second = rpc
        .get_multiple_accounts(&second_keys, u16::MAX, Some(first.slot))
        .await?;
    let spec_account = required_batch(&second, 0, "SourceSpecV1")?;
    authenticate_record_pair(
        registry,
        spec,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_id,
        spec_account,
        second.accounts.get(1).and_then(Option::as_ref),
        &rent,
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&spec_account.data)
        .map_err(|error| RelayerError::config(format!("SourceSpecV1 refused: {error:?}")))?;
    if source.access_profile() != SourceAccessProfile::RelayedObservationRecord {
        return Err(RelayerError::config(
            "SourceSpecV1 does not select RelayedObservationRecord",
        ));
    }
    let window_account = required_batch(&second, 2, "WindowSpecV1")?;
    authenticate_record_pair(
        registry,
        window,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_id,
        window_account,
        second.accounts.get(3).and_then(Option::as_ref),
        &rent,
        WINDOW_SPEC_BYTES,
    )?;
    let window_value = WindowSpecV1::decode(&window_account.data)
        .map_err(|error| RelayerError::config(format!("WindowSpecV1 refused: {error:?}")))?;
    window_value
        .validate_source(
            dclutch_source_contract::ContentId::new(source_spec_id)
                .map_err(|error| RelayerError::config(format!("source identity: {error:?}")))?,
        )
        .map_err(|error| RelayerError::config(format!("Window/Source join: {error:?}")))?;

    let provider_release_id = source.provider_release_id().to_bytes();
    let provider = record_pair(registry, PROVIDER_RELEASE_SCHEMA_ID_V1, provider_release_id);
    let third_keys = [provider.raw, provider.staging];
    let third = rpc
        .get_multiple_accounts(&third_keys, u16::MAX, Some(second.slot))
        .await?;
    let provider_account = required_batch(&third, 0, "ProviderReleaseV1")?;
    authenticate_record_pair(
        registry,
        provider,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_release_id,
        provider_account,
        third.accounts.get(1).and_then(Option::as_ref),
        &rent,
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider_value = ProviderReleaseV1::decode(&provider_account.data)
        .map_err(|error| RelayerError::config(format!("ProviderReleaseV1 refused: {error:?}")))?;
    if provider_value.provider_family_id().to_bytes() != RELAYED_FAMILY_RELEASE_ID_V1
        || provider_value.transport_profile_id().to_bytes()
            != RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1
        || provider_value.decoding_rules_id().to_bytes() != artifact.decoding_rules_id
    {
        return Err(RelayerError::config(
            "ProviderReleaseV1 family/transport/decoding-rules join refused",
        ));
    }
    let relayer_key_set_id = provider_value.provider_deployment_release_id().to_bytes();
    let key_set = record_pair(
        registry,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        relayer_key_set_id,
    );
    let adapter = record_pair(
        registry,
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
        artifact.decoding_rules_id,
    );
    let fourth_keys = [key_set.raw, key_set.staging, adapter.raw, adapter.staging];
    let fourth = rpc
        .get_multiple_accounts(&fourth_keys, u16::MAX, Some(third.slot))
        .await?;
    let key_set_account = required_batch(&fourth, 0, "RelayerKeySetV1")?;
    authenticate_record_pair(
        registry,
        key_set,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        relayer_key_set_id,
        key_set_account,
        fourth.accounts.get(1).and_then(Option::as_ref),
        &rent,
        RELAYER_KEY_SET_BYTES,
    )?;
    let key_set_value = RelayerKeySetV1::decode(&key_set_account.data)
        .map_err(|error| RelayerError::config(format!("RelayerKeySetV1 refused: {error:?}")))?;
    key_set_value
        .require_member(&artifact.signer)
        .map_err(|error| {
            RelayerError::config(format!("artifact signer is not release-pinned: {error:?}"))
        })?;
    if key_set.raw != submit.relayer_key_set
        || key_set.staging != submit.relayer_key_set_staging_vacancy
    {
        return Err(RelayerError::config(
            "the config's relayer-key-set addresses differ from the authenticated Source graph",
        ));
    }
    let adapter_account = required_batch(&fourth, 2, "RelayedAdapterConfigV1")?;
    authenticate_record_pair(
        registry,
        adapter,
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
        artifact.decoding_rules_id,
        adapter_account,
        fourth.accounts.get(3).and_then(Option::as_ref),
        &rent,
        RELAYED_ADAPTER_CONFIG_BYTES,
    )?;
    let adapter_value = RelayedAdapterConfigV1::decode(&adapter_account.data).map_err(|error| {
        RelayerError::config(format!("RelayedAdapterConfigV1 refused: {error:?}"))
    })?;
    adapter_value
        .require_window_admits_skew(window_value.max_age_seconds())
        .map_err(|error| RelayerError::config(format!("window/skew join refused: {error:?}")))?;
    if adapter_value.account_set_id() != artifact.account_set_id
        || configured_set(config, &artifact.set_name)?.account_set_id != artifact.account_set_id
    {
        return Err(RelayerError::config(
            "authenticated adapter account_set_id differs from artifact/config",
        ));
    }

    let (record, pda_bump) = crate::txn::derive_record_address(
        submit.relay_program_id,
        submit.market,
        submit.generation,
        artifact.account_set_id,
        artifact.observed_slot,
    );
    let keys = FrameKeysV1 {
        worker,
        market: submit.market,
        core,
        activation,
        record,
        material,
        spec,
        provider,
        window,
        key_set,
        adapter,
        beneficiary,
    };
    let frame_keys = ordered_frame_keys(&keys)?;
    let final_read = rpc
        .get_multiple_accounts(&frame_keys, u16::MAX, Some(fourth.slot))
        .await?;
    let snapshot = FrameSnapshotV1 {
        slot: final_read.slot,
        keys: frame_keys,
        accounts: final_read.accounts,
    };
    authenticate_final_frame(
        submit,
        artifact,
        &keys,
        &snapshot,
        source_material_id,
        source_spec_id,
        provider_release_id,
        relayer_key_set_id,
        key_set_value.seal_threshold(),
    )?;
    let record_width = relayed_observation_record_bytes_v1(artifact.set_count)
        .map_err(|error| RelayerError::wire("record width", error))?;
    let final_rent = decode_rent(required_snapshot(&snapshot, 18, "Rent sysvar")?)?;
    let record_rent_minimum = final_rent.minimum_balance(record_width);
    let worker_lamports = required_snapshot(&snapshot, 0, "worker")?.lamports;
    let record_account = snapshot.accounts.get(4).and_then(Option::as_ref);
    let (record_lamports, already_complete) = authenticate_record_prestate(
        record_account,
        submit.relay_program_id,
        &keys,
        artifact,
        submit.generation,
        source_material_id,
        provider_release_id,
        relayer_key_set_id,
        key_set_value.seal_threshold(),
        required_snapshot(&snapshot, 19, "Clock sysvar")?,
    )?;
    if already_complete && !final_rent.is_exempt(record_lamports, record_width) {
        return Err(RelayerError::config(
            "exact existing Collecting record is not rent exempt",
        ));
    }
    Ok(AuthenticatedCreateV1 {
        snapshot,
        keys,
        generation: submit.generation,
        source_material_id,
        source_spec_id,
        provider_release_id,
        relayer_key_set_id,
        account_set_id: artifact.account_set_id,
        observed_slot: artifact.observed_slot,
        set_count: artifact.set_count,
        seal_threshold: key_set_value.seal_threshold(),
        pda_bump,
        record_width,
        record_rent_minimum,
        record_lamports,
        worker_lamports,
        already_complete,
    })
}

fn required_batch<'a>(
    batch: &'a BatchRead,
    index: usize,
    name: &str,
) -> Result<&'a ObservedAccount> {
    batch
        .accounts
        .get(index)
        .and_then(Option::as_ref)
        .ok_or_else(|| RelayerError::config(format!("finalized {name} account is absent")))
}

fn required_snapshot<'a>(
    snapshot: &'a FrameSnapshotV1,
    index: usize,
    name: &str,
) -> Result<&'a ObservedAccount> {
    snapshot
        .accounts
        .get(index)
        .and_then(Option::as_ref)
        .ok_or_else(|| RelayerError::config(format!("finalized {name} account is absent")))
}

fn authenticate_market(
    submit: &SubmitConfig,
    account: &ObservedAccount,
    state: CoreState,
) -> Result<()> {
    let expected = derive_pda(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        account.owner,
    );
    if account.executable
        || account.data_len != account.data.len() as u64
        || state.phase != Phase::Open
        || state.readiness != Readiness::Consumed
        || state.identity.generation != submit.generation
        || state.identity.market_id.to_bytes() != submit.market
        || state.identity.resolution_policy.to_bytes() == [0; ID_BYTES]
        || expected != submit.market
    {
        return Err(RelayerError::config(
            "Core Market owner/address/phase/generation identity refused",
        ));
    }
    Ok(())
}

fn derive_pda(seeds: &[&[u8]], program: [u8; ID_BYTES]) -> [u8; ID_BYTES] {
    Address::find_program_address(seeds, &Address::from(program))
        .0
        .to_bytes()
}

fn record_pair(
    registry: [u8; ID_BYTES],
    schema: [u8; ID_BYTES],
    digest: [u8; ID_BYTES],
) -> RecordPairV1 {
    RecordPairV1 {
        raw: derive_pda(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            registry,
        ),
        staging: derive_pda(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            registry,
        ),
    }
}

fn decode_rent(account: &ObservedAccount) -> Result<Rent> {
    let sysvar_owner = parse_id32("Sysvar program", SYSVAR_PROGRAM_ID_BASE58)?;
    if account.owner != sysvar_owner
        || account.executable
        || account.data_len != account.data.len() as u64
    {
        return Err(RelayerError::config("canonical Rent sysvar refused"));
    }
    let rent: Rent = bincode::deserialize(&account.data)
        .map_err(|error| RelayerError::config(format!("Rent sysvar decode refused: {error}")))?;
    let canonical_width = bincode::serialized_size(&rent)
        .map_err(|error| RelayerError::config(format!("Rent sysvar width refused: {error}")))?;
    if canonical_width != account.data_len {
        return Err(RelayerError::config("Rent sysvar has trailing bytes"));
    }
    Ok(rent)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_record_pair(
    registry: [u8; ID_BYTES],
    pair: RecordPairV1,
    schema: [u8; ID_BYTES],
    expected_digest: [u8; ID_BYTES],
    raw: &ObservedAccount,
    staging: Option<&ObservedAccount>,
    rent: &Rent,
    exact_width: usize,
) -> Result<()> {
    let canonical = record_pair(registry, schema, expected_digest);
    let body_digest: [u8; ID_BYTES] = Sha256::digest(&raw.data).into();
    if pair.raw != canonical.raw
        || pair.staging != canonical.staging
        || raw.owner != registry
        || raw.executable
        || raw.data.len() != exact_width
        || raw.data_len != exact_width as u64
        || body_digest != expected_digest
        || !rent.is_exempt(raw.lamports, exact_width)
    {
        return Err(RelayerError::config(
            "finalized Registry raw record identity/content/rent refused",
        ));
    }
    authenticate_vacancy(staging, "Registry staging cursor")
}

fn authenticate_vacancy(account: Option<&ObservedAccount>, name: &str) -> Result<()> {
    if let Some(account) = account
        && (account.owner != SYSTEM_PROGRAM_ID
            || account.executable
            || account.data_len != 0
            || !account.data.is_empty())
    {
        return Err(RelayerError::config(format!(
            "{name} is not an exact System-owned vacancy"
        )));
    }
    Ok(())
}

fn ordered_frame_keys(keys: &FrameKeysV1) -> Result<Vec<[u8; ID_BYTES]>> {
    let mut addresses = Vec::with_capacity(21);
    for role in relay_frame_roles_v1(RelayFrameKindV1::CreateRecord) {
        let key = match role.name() {
            RelayAccountNameV1::Worker => keys.worker,
            RelayAccountNameV1::Market => keys.market,
            RelayAccountNameV1::CoreProgram => keys.core,
            RelayAccountNameV1::RegistryActivation => keys.activation,
            RelayAccountNameV1::Record => keys.record,
            RelayAccountNameV1::SourceMaterial => keys.material.raw,
            RelayAccountNameV1::SourceMaterialStagingVacancy => keys.material.staging,
            RelayAccountNameV1::SourceSpec => keys.spec.raw,
            RelayAccountNameV1::SourceSpecStagingVacancy => keys.spec.staging,
            RelayAccountNameV1::ProviderRelease => keys.provider.raw,
            RelayAccountNameV1::ProviderReleaseStagingVacancy => keys.provider.staging,
            RelayAccountNameV1::WindowSpec => keys.window.raw,
            RelayAccountNameV1::WindowSpecStagingVacancy => keys.window.staging,
            RelayAccountNameV1::RelayerKeySet => keys.key_set.raw,
            RelayAccountNameV1::RelayerKeySetStagingVacancy => keys.key_set.staging,
            RelayAccountNameV1::AdapterConfig => keys.adapter.raw,
            RelayAccountNameV1::AdapterConfigStagingVacancy => keys.adapter.staging,
            RelayAccountNameV1::RentBeneficiary => keys.beneficiary,
            RelayAccountNameV1::RentSysvar => RENT_SYSVAR_ID,
            RelayAccountNameV1::ClockSysvar => CLOCK_SYSVAR_ID,
            RelayAccountNameV1::SystemProgram => SYSTEM_PROGRAM_ID,
            other => {
                return Err(RelayerError::config(format!(
                    "unexpected {other:?} in canonical CreateRecord frame"
                )));
            }
        };
        addresses.push(key);
    }
    let observed: Vec<_> = relay_frame_roles_v1(RelayFrameKindV1::CreateRecord)
        .iter()
        .zip(&addresses)
        .map(|(role, key)| RelayAccountPrivilegeV1 {
            key: *key,
            is_signer: role.is_signer(),
            is_writable: role.is_writable(),
        })
        .collect();
    validate_relay_frame_v1(RelayFrameKindV1::CreateRecord, &observed)
        .map_err(|error| RelayerError::wire("canonical CreateRecord account frame", error))?;
    Ok(addresses)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_final_frame(
    submit: &SubmitConfig,
    artifact: &ArtifactRouteV1,
    keys: &FrameKeysV1,
    snapshot: &FrameSnapshotV1,
    source_material_id: [u8; ID_BYTES],
    source_spec_id: [u8; ID_BYTES],
    provider_release_id: [u8; ID_BYTES],
    relayer_key_set_id: [u8; ID_BYTES],
    seal_threshold: u8,
) -> Result<()> {
    let expected_keys = ordered_frame_keys(keys)?;
    if snapshot.slot == 0
        || snapshot.keys != expected_keys
        || snapshot.accounts.len() != expected_keys.len()
    {
        return Err(RelayerError::config(
            "authoritative CreateRecord snapshot geometry refused",
        ));
    }
    let sysvar_owner = parse_id32("Sysvar program", SYSVAR_PROGRAM_ID_BASE58)?;
    let native_loader = parse_id32("Native Loader", NATIVE_LOADER_ID_BASE58)?;
    let worker = required_snapshot(snapshot, 0, "worker")?;
    if worker.owner != SYSTEM_PROGRAM_ID
        || worker.executable
        || worker.data_len != 0
        || !worker.data.is_empty()
    {
        return Err(RelayerError::config(
            "worker is not one canonical System-owned fee-payer account",
        ));
    }
    let market_account = required_snapshot(snapshot, 1, "Core Market")?;
    let market = CoreState::decode(&market_account.data)
        .map_err(|error| RelayerError::config(format!("Core Market refused: {error:?}")))?;
    authenticate_market(submit, market_account, market)?;
    if market.identity.resolution_policy.to_bytes() != source_material_id
        || market.rent_beneficiary.to_bytes() != keys.beneficiary
        || market_account.owner != keys.core
    {
        return Err(RelayerError::config(
            "Core Market Source/beneficiary/Core join refused",
        ));
    }
    let core = required_snapshot(snapshot, 2, "Core program")?;
    if !core.executable {
        return Err(RelayerError::config("Core program is not executable"));
    }
    let rent_account = required_snapshot(snapshot, 18, "Rent sysvar")?;
    let rent = decode_rent(rent_account)?;
    if !rent.is_exempt(market_account.lamports, market_account.data.len()) {
        return Err(RelayerError::config("Core Market is not rent exempt"));
    }

    let registry = market.identity.registry_program.to_bytes();
    let release_set = market.identity.selected_release_set.to_bytes();
    let activation = required_snapshot(snapshot, 3, "Registry activation")?;
    if activation.owner != registry
        || activation.executable
        || activation.data_len != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 as u64
        || activation.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || !rent.is_exempt(activation.lamports, activation.data.len())
    {
        return Err(RelayerError::config(
            "Registry activation owner/width/rent refused",
        ));
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation.data)
        .map_err(|error| RelayerError::config(format!("Registry activation refused: {error:?}")))?;
    let selected_resolution = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|error| {
            RelayerError::config(format!("Resolution activation refused: {error:?}"))
        })?;
    let selected_core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|error| RelayerError::config(format!("Core activation refused: {error:?}")))?;
    if activated
        .execution_release_set_id()
        .map_err(|error| RelayerError::config(format!("release-set identity refused: {error:?}")))?
        .to_bytes()
        != release_set
        || selected_resolution.release().program().to_bytes() != submit.relay_program_id
        || selected_core.release().program().to_bytes() != keys.core
    {
        return Err(RelayerError::config(
            "activated Core/Resolution deployment identities refused",
        ));
    }

    let material_account = required_snapshot(snapshot, 5, "SourceMaterialV3")?;
    authenticate_record_pair(
        registry,
        keys.material,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        source_material_id,
        material_account,
        snapshot.accounts.get(6).and_then(Option::as_ref),
        &rent,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material = SourceMaterialV3::decode(&material_account.data)
        .map_err(|error| RelayerError::config(format!("SourceMaterialV3 refused: {error:?}")))?;
    if material.primary_source_spec().to_bytes() != source_spec_id {
        return Err(RelayerError::config(
            "SourceMaterialV3 source-spec join refused",
        ));
    }
    let window_id = material.window_spec().to_bytes();

    let spec_account = required_snapshot(snapshot, 7, "SourceSpecV1")?;
    authenticate_record_pair(
        registry,
        keys.spec,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_id,
        spec_account,
        snapshot.accounts.get(8).and_then(Option::as_ref),
        &rent,
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&spec_account.data)
        .map_err(|error| RelayerError::config(format!("SourceSpecV1 refused: {error:?}")))?;
    if source.access_profile() != SourceAccessProfile::RelayedObservationRecord
        || source.provider_release_id().to_bytes() != provider_release_id
    {
        return Err(RelayerError::config("SourceSpecV1 release join refused"));
    }

    let provider_account = required_snapshot(snapshot, 9, "ProviderReleaseV1")?;
    authenticate_record_pair(
        registry,
        keys.provider,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_release_id,
        provider_account,
        snapshot.accounts.get(10).and_then(Option::as_ref),
        &rent,
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider = ProviderReleaseV1::decode(&provider_account.data)
        .map_err(|error| RelayerError::config(format!("ProviderReleaseV1 refused: {error:?}")))?;
    if provider.provider_family_id().to_bytes() != RELAYED_FAMILY_RELEASE_ID_V1
        || provider.transport_profile_id().to_bytes() != RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1
        || provider.provider_deployment_release_id().to_bytes() != relayer_key_set_id
        || provider.decoding_rules_id().to_bytes() != artifact.decoding_rules_id
    {
        return Err(RelayerError::config("ProviderReleaseV1 joins refused"));
    }

    let window_account = required_snapshot(snapshot, 11, "WindowSpecV1")?;
    authenticate_record_pair(
        registry,
        keys.window,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_id,
        window_account,
        snapshot.accounts.get(12).and_then(Option::as_ref),
        &rent,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_account.data)
        .map_err(|error| RelayerError::config(format!("WindowSpecV1 refused: {error:?}")))?;
    window
        .validate_source(
            dclutch_source_contract::ContentId::new(source_spec_id)
                .map_err(|error| RelayerError::config(format!("source identity: {error:?}")))?,
        )
        .map_err(|error| RelayerError::config(format!("Window/Source join refused: {error:?}")))?;

    let key_set_account = required_snapshot(snapshot, 13, "RelayerKeySetV1")?;
    authenticate_record_pair(
        registry,
        keys.key_set,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        relayer_key_set_id,
        key_set_account,
        snapshot.accounts.get(14).and_then(Option::as_ref),
        &rent,
        RELAYER_KEY_SET_BYTES,
    )?;
    let key_set = RelayerKeySetV1::decode(&key_set_account.data)
        .map_err(|error| RelayerError::config(format!("RelayerKeySetV1 refused: {error:?}")))?;
    if key_set.seal_threshold() != seal_threshold
        || key_set.require_member(&artifact.signer).is_err()
    {
        return Err(RelayerError::config("RelayerKeySetV1 threshold changed"));
    }

    let adapter_account = required_snapshot(snapshot, 15, "RelayedAdapterConfigV1")?;
    authenticate_record_pair(
        registry,
        keys.adapter,
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
        artifact.decoding_rules_id,
        adapter_account,
        snapshot.accounts.get(16).and_then(Option::as_ref),
        &rent,
        RELAYED_ADAPTER_CONFIG_BYTES,
    )?;
    let adapter = RelayedAdapterConfigV1::decode(&adapter_account.data).map_err(|error| {
        RelayerError::config(format!("RelayedAdapterConfigV1 refused: {error:?}"))
    })?;
    adapter
        .require_window_admits_skew(window.max_age_seconds())
        .map_err(|error| RelayerError::config(format!("window/skew join refused: {error:?}")))?;
    if adapter.account_set_id() != artifact.account_set_id {
        return Err(RelayerError::config("adapter account-set identity refused"));
    }

    let clock = required_snapshot(snapshot, 19, "Clock sysvar")?;
    if clock.owner != sysvar_owner
        || clock.executable
        || crate::chain::clock_unix_timestamp(&clock.data).is_none()
        || clock.data_len != clock.data.len() as u64
    {
        return Err(RelayerError::config("canonical Clock sysvar refused"));
    }
    let system = required_snapshot(snapshot, 20, "System program")?;
    if !system.executable || system.owner != native_loader {
        return Err(RelayerError::config("canonical System program refused"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_record_prestate(
    account: Option<&ObservedAccount>,
    relay_program: [u8; ID_BYTES],
    keys: &FrameKeysV1,
    artifact: &ArtifactRouteV1,
    generation: u64,
    source_material_id: [u8; ID_BYTES],
    provider_release_id: [u8; ID_BYTES],
    relayer_key_set_id: [u8; ID_BYTES],
    seal_threshold: u8,
    clock: &ObservedAccount,
) -> Result<(u64, bool)> {
    let Some(account) = account else {
        return Ok((0, false));
    };
    if account.owner == SYSTEM_PROGRAM_ID
        && !account.executable
        && account.data_len == 0
        && account.data.is_empty()
    {
        return Ok((account.lamports, false));
    }
    let exact_width = relayed_observation_record_bytes_v1(artifact.set_count)
        .map_err(|error| RelayerError::wire("record prestate width", error))?;
    if account.owner != relay_program
        || account.executable
        || account.data.len() != exact_width
        || account.data_len != exact_width as u64
    {
        return Err(RelayerError::config(
            "record PDA is neither vacant nor the exact predicted relay record",
        ));
    }
    let view = RelayedObservationRecordViewV1::decode(&account.data)
        .map_err(|error| RelayerError::wire("existing observation record", error))?;
    let created = view
        .created_unix_seconds()
        .map_err(|error| RelayerError::wire("existing record creation time", error))?;
    let now = crate::chain::clock_unix_timestamp(&clock.data)
        .ok_or_else(|| RelayerError::config("canonical Clock sysvar refused"))?;
    let mut seed = [0u8; SET_DIGEST_SEED_PREIMAGE_BYTES];
    encode_set_digest_seed_preimage_v1(&mut seed, artifact.account_set_id, artifact.observed_slot)
        .map_err(|error| RelayerError::wire("record seed digest", error))?;
    let expected_seed: [u8; ID_BYTES] = Sha256::digest(seed).into();
    if view
        .phase()
        .map_err(|error| RelayerError::wire("record phase", error))?
        != RelayedRecordPhaseV1::Collecting
        || view
            .market()
            .map_err(|error| RelayerError::wire("record Market", error))?
            != keys.market
        || view
            .generation()
            .map_err(|error| RelayerError::wire("record generation", error))?
            != generation
        || view
            .source_material_id()
            .map_err(|error| RelayerError::wire("record material", error))?
            != source_material_id
        || view
            .account_set_id()
            .map_err(|error| RelayerError::wire("record set", error))?
            != artifact.account_set_id
        || view
            .provider_release_id()
            .map_err(|error| RelayerError::wire("record provider", error))?
            != provider_release_id
        || view
            .relayer_key_set_id()
            .map_err(|error| RelayerError::wire("record key set", error))?
            != relayer_key_set_id
        || view
            .observed_cluster_id()
            .map_err(|error| RelayerError::wire("record cluster", error))?
            != SOLANA_MAINNET_GENESIS_HASH_V1
        || view
            .observed_slot()
            .map_err(|error| RelayerError::wire("record observed slot", error))?
            != artifact.observed_slot
        || view
            .rent_credit_beneficiary()
            .map_err(|error| RelayerError::wire("record beneficiary", error))?
            != keys.beneficiary
        || view
            .set_count()
            .map_err(|error| RelayerError::wire("record set count", error))?
            != artifact.set_count
        || view
            .seal_threshold()
            .map_err(|error| RelayerError::wire("record threshold", error))?
            != seal_threshold
        || view
            .filled_count()
            .map_err(|error| RelayerError::wire("record filled count", error))?
            != 0
        || view
            .seal_count()
            .map_err(|error| RelayerError::wire("record seal count", error))?
            != 0
        || view
            .sealed_by_bitmap()
            .map_err(|error| RelayerError::wire("record seal bitmap", error))?
            != 0
        || view
            .sealed_unix_seconds()
            .map_err(|error| RelayerError::wire("record sealed time", error))?
            != 0
        || view
            .set_digest()
            .map_err(|error| RelayerError::wire("record seed digest", error))?
            != expected_seed
        || created <= 0
        || created > now
    {
        return Err(RelayerError::config(
            "existing Collecting record differs from the predicted exact poststate",
        ));
    }
    Ok((account.lamports, true))
}

fn build_create_instruction(
    relay_program_id: [u8; ID_BYTES],
    authenticated: &AuthenticatedCreateV1,
) -> Result<Instruction> {
    let request = CreateRecordInstructionV1::new(
        authenticated.generation,
        authenticated.observed_slot,
        authenticated.set_count,
        authenticated.seal_threshold,
        authenticated.pda_bump,
        authenticated.source_material_id,
        authenticated.source_spec_id,
        authenticated.keys.beneficiary,
    )
    .map_err(|error| RelayerError::wire("CreateRecord instruction", error))?;
    let addresses = ordered_frame_keys(&authenticated.keys)?;
    let roles = relay_frame_roles_v1(RelayFrameKindV1::CreateRecord);
    let accounts = roles
        .iter()
        .zip(addresses)
        .map(|(role, key)| AccountMeta {
            pubkey: Address::from(key),
            is_signer: role.is_signer(),
            is_writable: role.is_writable(),
        })
        .collect();
    Ok(Instruction {
        program_id: Address::from(relay_program_id),
        accounts,
        data: request
            .to_bytes()
            .map_err(|error| RelayerError::wire("CreateRecord instruction bytes", error))?
            .to_vec(),
    })
}

fn build_plan(
    submit: &SubmitConfig,
    authenticated: &AuthenticatedCreateV1,
    instruction: &Instruction,
) -> Result<CreateRecordPlanV1> {
    let prestate = prestate_digest(&authenticated.snapshot);
    let top_up = authenticated
        .record_rent_minimum
        .saturating_sub(authenticated.record_lamports);
    let priority = priority_fee_ceiling(submit)?;
    let minimum_before_base_fee = top_up
        .checked_add(priority)
        .ok_or_else(|| RelayerError::config("rent and priority-fee ceiling overflowed"))?;
    let packet = packet_bound(submit, authenticated.keys.worker, instruction)?;
    let roles = relay_frame_roles_v1(RelayFrameKindV1::CreateRecord);
    if instruction.accounts.len() != roles.len() {
        return Err(RelayerError::config(
            "CreateRecord instruction did not preserve the canonical role count",
        ));
    }
    let accounts = instruction
        .accounts
        .iter()
        .zip(roles)
        .map(|(meta, role)| PlannedAccountMetaV1 {
            role: role_name(role.name()),
            address: meta.pubkey.to_string(),
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        })
        .collect();
    Ok(CreateRecordPlanV1 {
        format: PLAN_FORMAT_V1,
        status: if authenticated.already_complete {
            "already-finalized"
        } else {
            "unsigned"
        },
        finalized_snapshot_slot: authenticated.snapshot.slot,
        submit_cluster_genesis: base58(&SOLANA_DEVNET_GENESIS_HASH_V1),
        market: base58(&authenticated.keys.market),
        generation: authenticated.generation,
        account_set_id: to_hex(&authenticated.account_set_id),
        observed_slot: authenticated.observed_slot,
        record: base58(&authenticated.keys.record),
        prestate_sha256: to_hex(&prestate),
        instruction: PlannedInstructionV1 {
            program_id: instruction.program_id.to_string(),
            accounts,
            data_base64: BASE64.encode(&instruction.data),
        },
        funding: FundingPlanV1 {
            worker_balance_lamports: authenticated.worker_lamports,
            existing_record_lamports: authenticated.record_lamports,
            record_rent_minimum_lamports: authenticated.record_rent_minimum,
            record_top_up_lamports: top_up,
            priority_fee_ceiling_lamports: priority,
            minimum_before_base_fee_lamports: minimum_before_base_fee,
            base_fee: "quoted from finalized getFeeForMessage immediately before signing",
        },
        packet_bound: packet,
        expected_poststate: ExpectedPoststateV1 {
            owner: base58(&submit.relay_program_id),
            phase: "Collecting",
            set_count: authenticated.set_count,
            seal_threshold: authenticated.seal_threshold,
            filled_count: 0,
            seal_count: 0,
            minimum_lamports: authenticated.record_rent_minimum,
            created_unix_seconds: "positive and no later than finalized Clock",
        },
    })
}

fn priority_fee_ceiling(submit: &SubmitConfig) -> Result<u64> {
    let price = submit.compute_unit_price_micro_lamports.unwrap_or(0);
    if price == 0 {
        return Ok(0);
    }
    // When no explicit limit instruction exists the runtime transaction ceiling
    // is the conservative bound. This figure is a plan ceiling; execution uses
    // getFeeForMessage for the exact debit.
    let units = u128::from(submit.compute_unit_limit.unwrap_or(1_400_000));
    let micro = units
        .checked_mul(u128::from(price))
        .ok_or_else(|| RelayerError::config("priority-fee ceiling overflowed"))?;
    u64::try_from(
        micro
            .checked_add(MICRO_LAMPORTS_PER_LAMPORT - 1)
            .ok_or_else(|| RelayerError::config("priority-fee ceiling overflowed"))?
            / MICRO_LAMPORTS_PER_LAMPORT,
    )
    .map_err(|_| RelayerError::config("priority-fee ceiling does not fit u64"))
}

fn compute_budget_instructions(submit: &SubmitConfig) -> Vec<Instruction> {
    let mut instructions = Vec::with_capacity(2);
    if let Some(limit) = submit.compute_unit_limit {
        let mut data = Vec::with_capacity(5);
        data.push(COMPUTE_BUDGET_SET_UNIT_LIMIT_TAG);
        data.extend_from_slice(&limit.to_le_bytes());
        instructions.push(Instruction {
            program_id: Address::from(COMPUTE_BUDGET_PROGRAM_ID),
            accounts: Vec::new(),
            data,
        });
    }
    if let Some(price) = submit.compute_unit_price_micro_lamports {
        let mut data = Vec::with_capacity(9);
        data.push(COMPUTE_BUDGET_SET_UNIT_PRICE_TAG);
        data.extend_from_slice(&price.to_le_bytes());
        instructions.push(Instruction {
            program_id: Address::from(COMPUTE_BUDGET_PROGRAM_ID),
            accounts: Vec::new(),
            data,
        });
    }
    instructions
}

fn transaction_instructions(submit: &SubmitConfig, instruction: &Instruction) -> Vec<Instruction> {
    let mut instructions = compute_budget_instructions(submit);
    instructions.push(instruction.clone());
    instructions
}

fn lookup_table(submit: &SubmitConfig) -> Option<AddressLookupTableAccount> {
    submit
        .address_lookup_table
        .as_ref()
        .map(|table| AddressLookupTableAccount {
            key: Address::from(table.key),
            addresses: table.addresses.iter().copied().map(Address::from).collect(),
        })
}

fn compile_legacy(
    submit: &SubmitConfig,
    payer: [u8; ID_BYTES],
    instruction: &Instruction,
    blockhash: [u8; ID_BYTES],
) -> VersionedMessage {
    let instructions = transaction_instructions(submit, instruction);
    VersionedMessage::Legacy(legacy::Message::new_with_blockhash(
        &instructions,
        Some(&Address::from(payer)),
        &Hash::new_from_array(blockhash),
    ))
}

fn compile_v0(
    submit: &SubmitConfig,
    payer: [u8; ID_BYTES],
    instruction: &Instruction,
    blockhash: [u8; ID_BYTES],
) -> Result<VersionedMessage> {
    let instructions = transaction_instructions(submit, instruction);
    let tables: Vec<_> = lookup_table(submit).into_iter().collect();
    let message = v0::Message::try_compile(
        &Address::from(payer),
        &instructions,
        &tables,
        Hash::new_from_array(blockhash),
    )
    .map_err(|error| RelayerError::config(format!("could not compile v0 CreateRecord: {error}")))?;
    if message.address_table_lookups.is_empty() {
        return Err(RelayerError::config(
            "configured address lookup table routes no CreateRecord account",
        ));
    }
    Ok(VersionedMessage::V0(message))
}

fn wire_size(message: VersionedMessage) -> Result<usize> {
    Ok(serialize_transaction(&sign_transaction(message, [0; 64]))?.len())
}

fn packet_bound(
    submit: &SubmitConfig,
    payer: [u8; ID_BYTES],
    instruction: &Instruction,
) -> Result<PacketBoundV1> {
    let dummy_hash = [7; ID_BYTES];
    let legacy_bytes = wire_size(compile_legacy(submit, payer, instruction, dummy_hash))?;
    let (selected, v0_bytes, routed) = if submit.address_lookup_table.is_some() {
        let v0 = compile_v0(submit, payer, instruction, dummy_hash)?;
        let routed = match &v0 {
            VersionedMessage::V0(message) => message
                .address_table_lookups
                .iter()
                .try_fold(0usize, |total, lookup| {
                    total
                        .checked_add(lookup.writable_indexes.len())
                        .and_then(|value| value.checked_add(lookup.readonly_indexes.len()))
                })
                .ok_or_else(|| RelayerError::config("lookup address count overflowed"))?,
            _ => 0,
        };
        let bytes = wire_size(v0)?;
        ("v0", bytes, routed)
    } else {
        ("legacy", 0, 0)
    };
    let selected_bytes = if selected == "v0" {
        v0_bytes
    } else {
        legacy_bytes
    };
    if selected_bytes > SOLANA_PACKET_DATA_BYTES {
        return Err(RelayerError::config(format!(
            "selected {selected} CreateRecord wire is {selected_bytes} bytes, over Solana's {SOLANA_PACKET_DATA_BYTES}-byte packet bound"
        )));
    }
    Ok(PacketBoundV1 {
        selected_message_version: selected,
        legacy_packet_bytes: legacy_bytes,
        v0_packet_bytes: v0_bytes,
        maximum_packet_bytes: SOLANA_PACKET_DATA_BYTES,
        lookup_table_addresses: routed,
    })
}

fn compile_selected_message(
    submit: &SubmitConfig,
    payer: [u8; ID_BYTES],
    instruction: &Instruction,
    blockhash: [u8; ID_BYTES],
) -> Result<VersionedMessage> {
    if submit.address_lookup_table.is_some() {
        compile_v0(submit, payer, instruction, blockhash)
    } else {
        Ok(compile_legacy(submit, payer, instruction, blockhash))
    }
}

fn prestate_digest(snapshot: &FrameSnapshotV1) -> [u8; ID_BYTES] {
    let mut hash = Sha256::new();
    hash.update(PRESTATE_DOMAIN_V1);
    hash.update((snapshot.keys.len() as u64).to_le_bytes());
    for (key, account) in snapshot.keys.iter().zip(&snapshot.accounts) {
        hash.update(key);
        match account {
            None => hash.update([0]),
            Some(account) => {
                hash.update([1]);
                hash.update(account.lamports.to_le_bytes());
                hash.update(account.owner);
                hash.update([u8::from(account.executable)]);
                hash.update(account.data_len.to_le_bytes());
                hash.update((account.data.len() as u64).to_le_bytes());
                hash.update(&account.data);
            }
        }
    }
    hash.finalize().into()
}

fn persist_plan(slot_dir: &Path, plan: &CreateRecordPlanV1) -> Result<PathBuf> {
    let digest_prefix = plan
        .prestate_sha256
        .get(..16)
        .ok_or_else(|| RelayerError::config("prestate digest is not canonical hex"))?;
    let path = slot_dir.join("keeper").join(format!(
        "plan-{}-{digest_prefix}.json",
        plan.finalized_snapshot_slot
    ));
    write_immutable_json(&path, plan)?;
    Ok(path)
}

fn persist_receipt(slot_dir: &Path, receipt: &CreateRecordReceiptV1) -> Result<PathBuf> {
    let digest_prefix = receipt
        .plan_prestate_sha256
        .get(..16)
        .ok_or_else(|| RelayerError::config("receipt prestate digest is not canonical hex"))?;
    let path = slot_dir.join("keeper").join(format!(
        "receipt-{}-{digest_prefix}.json",
        receipt.finalized_slot
    ));
    write_immutable_json(&path, receipt)?;
    Ok(path)
}

fn write_immutable_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| RelayerError::config("journal path has no parent"))?;
    std::fs::create_dir_all(parent).map_err(|source| RelayerError::io(parent, source))?;
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| RelayerError::Serialization(format!("keeper journal: {error}")))?;
    bytes.push(b'\n');
    match std::fs::read(path) {
        Ok(existing) if existing == bytes => return Ok(()),
        Ok(_) => {
            return Err(RelayerError::config(format!(
                "refusing to overwrite divergent keeper journal {}",
                path.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => return Err(RelayerError::io(path, source)),
    }
    let mut selected = None;
    for nonce in 0..100u8 {
        let candidate = parent.join(format!(
            ".{}.tmp-{}-{nonce}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("keeper"),
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                selected = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(RelayerError::io(&candidate, source)),
        }
    }
    let (temporary, mut file) = selected
        .ok_or_else(|| RelayerError::config("could not allocate a keeper journal temporary"))?;
    file.write_all(&bytes)
        .map_err(|source| RelayerError::io(&temporary, source))?;
    file.sync_all()
        .map_err(|source| RelayerError::io(&temporary, source))?;
    if path.exists() {
        let existing = std::fs::read(path).map_err(|source| RelayerError::io(path, source))?;
        std::fs::remove_file(&temporary).map_err(|source| RelayerError::io(&temporary, source))?;
        if existing == bytes {
            return Ok(());
        }
        return Err(RelayerError::config(format!(
            "refusing concurrent divergent keeper journal {}",
            path.display()
        )));
    }
    std::fs::rename(&temporary, path).map_err(|source| RelayerError::io(path, source))?;
    let directory = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|source| RelayerError::io(parent, source))?;
    directory
        .sync_all()
        .map_err(|source| RelayerError::io(parent, source))?;
    Ok(())
}

fn receipt_for(
    plan: &CreateRecordPlanV1,
    authenticated: &AuthenticatedCreateV1,
    signature: Option<String>,
    finalized_slot: u64,
    transaction_fee_lamports: u64,
) -> Result<CreateRecordReceiptV1> {
    let top_up = authenticated
        .record_rent_minimum
        .saturating_sub(authenticated.record_lamports);
    let total = top_up
        .checked_add(transaction_fee_lamports)
        .ok_or_else(|| RelayerError::config("receipt debit arithmetic overflowed"))?;
    Ok(CreateRecordReceiptV1 {
        format: RECEIPT_FORMAT_V1,
        plan_prestate_sha256: plan.prestate_sha256.clone(),
        record: base58(&authenticated.keys.record),
        transaction_signature: signature,
        finalized_slot,
        record_lamports: authenticated.record_lamports,
        record_rent_minimum_lamports: authenticated.record_rent_minimum,
        record_top_up_lamports: top_up,
        transaction_fee_lamports,
        total_worker_debit_lamports: total,
    })
}

async fn await_exact_poststate(
    rpc: &RpcClient,
    relay_program: [u8; ID_BYTES],
    authenticated: &AuthenticatedCreateV1,
    minimum_slot: u64,
) -> Result<(u64, u64)> {
    let artifact = ArtifactRouteV1 {
        set_name: String::new(),
        account_set_id: authenticated.account_set_id,
        observed_slot: authenticated.observed_slot,
        set_count: authenticated.set_count,
        decoding_rules_id: [0; ID_BYTES],
        signer: [1; ID_BYTES],
    };
    let read_keys = [authenticated.keys.record, CLOCK_SYSVAR_ID, RENT_SYSVAR_ID];
    for _ in 0..POSTSTATE_POLLS {
        let read = rpc
            .get_multiple_accounts(&read_keys, u16::MAX, Some(minimum_slot))
            .await?;
        let clock = read
            .accounts
            .get(1)
            .and_then(Option::as_ref)
            .ok_or_else(|| RelayerError::config("finalized Clock disappeared during resume"))?;
        let rent_account = read
            .accounts
            .get(2)
            .and_then(Option::as_ref)
            .ok_or_else(|| RelayerError::config("finalized Rent disappeared during resume"))?;
        let rent = decode_rent(rent_account)?;
        let record = read.accounts.first().and_then(Option::as_ref);
        let (lamports, complete) = authenticate_record_prestate(
            record,
            relay_program,
            &authenticated.keys,
            &artifact,
            authenticated.generation,
            authenticated.source_material_id,
            authenticated.provider_release_id,
            authenticated.relayer_key_set_id,
            authenticated.seal_threshold,
            clock,
        )?;
        if complete {
            if !rent.is_exempt(lamports, authenticated.record_width) {
                return Err(RelayerError::config(
                    "finalized CreateRecord poststate is not rent exempt",
                ));
            }
            return Ok((read.slot, lamports));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(RelayerError::config(
        "submitted CreateRecord did not reach the exact finalized Collecting poststate within 30 seconds",
    ))
}

const fn role_name(name: RelayAccountNameV1) -> &'static str {
    match name {
        RelayAccountNameV1::Worker => "worker",
        RelayAccountNameV1::Market => "market",
        RelayAccountNameV1::CoreProgram => "coreProgram",
        RelayAccountNameV1::RegistryActivation => "registryActivation",
        RelayAccountNameV1::Record => "record",
        RelayAccountNameV1::SourceMaterial => "sourceMaterial",
        RelayAccountNameV1::SourceMaterialStagingVacancy => "sourceMaterialStagingVacancy",
        RelayAccountNameV1::SourceSpec => "sourceSpec",
        RelayAccountNameV1::SourceSpecStagingVacancy => "sourceSpecStagingVacancy",
        RelayAccountNameV1::ProviderRelease => "providerRelease",
        RelayAccountNameV1::ProviderReleaseStagingVacancy => "providerReleaseStagingVacancy",
        RelayAccountNameV1::WindowSpec => "windowSpec",
        RelayAccountNameV1::WindowSpecStagingVacancy => "windowSpecStagingVacancy",
        RelayAccountNameV1::RelayerKeySet => "relayerKeySet",
        RelayAccountNameV1::RelayerKeySetStagingVacancy => "relayerKeySetStagingVacancy",
        RelayAccountNameV1::AdapterConfig => "adapterConfig",
        RelayAccountNameV1::AdapterConfigStagingVacancy => "adapterConfigStagingVacancy",
        RelayAccountNameV1::RentBeneficiary => "rentBeneficiary",
        RelayAccountNameV1::RentSysvar => "rentSysvar",
        RelayAccountNameV1::ClockSysvar => "clockSysvar",
        RelayAccountNameV1::InstructionsSysvar => "instructionsSysvar",
        RelayAccountNameV1::SystemProgram => "systemProgram",
        RelayAccountNameV1::SourceResolutionState => "sourceResolutionState",
        RelayAccountNameV1::ResolutionCertificate => "resolutionCertificate",
        RelayAccountNameV1::VenueArtifactRelease => "venueArtifactRelease",
        RelayAccountNameV1::VenueArtifactReleaseStagingVacancy => {
            "venueArtifactReleaseStagingVacancy"
        }
        RelayAccountNameV1::ProductRecord => "productRecord",
        RelayAccountNameV1::ProductRecordStagingVacancy => "productRecordStagingVacancy",
        RelayAccountNameV1::ResultDomain => "resultDomain",
        RelayAccountNameV1::ResultDomainStagingVacancy => "resultDomainStagingVacancy",
        RelayAccountNameV1::PortfolioRecord => "portfolioRecord",
        RelayAccountNameV1::PortfolioRecordStagingVacancy => "portfolioRecordStagingVacancy",
        RelayAccountNameV1::CapabilityManifest => "capabilityManifest",
        RelayAccountNameV1::CapabilityManifestStagingVacancy => "capabilityManifestStagingVacancy",
        RelayAccountNameV1::FailureFunding => "failureFunding",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PositionConfig;
    use dclutch_relay_contract::{
        SHA256_EMPTY_DIGEST,
        record::{
            RelayedRecordBindingV1, append_relayed_observation_in_place_v1,
            create_relayed_observation_record_into_v1, seal_relayed_observation_in_place_v1,
        },
        wire::AccountObservationV1,
    };
    use ed25519_dalek::{Signer as _, SigningKey};
    use tempfile::TempDir;

    fn account(owner: [u8; ID_BYTES], lamports: u64, data: Vec<u8>) -> ObservedAccount {
        ObservedAccount {
            lamports,
            owner,
            executable: false,
            data_len: data.len() as u64,
            data,
        }
    }

    fn frame_keys() -> FrameKeysV1 {
        FrameKeysV1 {
            worker: [1; ID_BYTES],
            market: [2; ID_BYTES],
            core: [3; ID_BYTES],
            activation: [4; ID_BYTES],
            record: [5; ID_BYTES],
            material: RecordPairV1 {
                raw: [6; ID_BYTES],
                staging: [7; ID_BYTES],
            },
            spec: RecordPairV1 {
                raw: [8; ID_BYTES],
                staging: [9; ID_BYTES],
            },
            provider: RecordPairV1 {
                raw: [10; ID_BYTES],
                staging: [11; ID_BYTES],
            },
            window: RecordPairV1 {
                raw: [12; ID_BYTES],
                staging: [13; ID_BYTES],
            },
            key_set: RecordPairV1 {
                raw: [14; ID_BYTES],
                staging: [15; ID_BYTES],
            },
            adapter: RecordPairV1 {
                raw: [16; ID_BYTES],
                staging: [17; ID_BYTES],
            },
            beneficiary: [18; ID_BYTES],
        }
    }

    fn submit() -> SubmitConfig {
        SubmitConfig {
            endpoint: "http://127.0.0.1:8899".to_owned(),
            expected_genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
            allow_public_submission: false,
            relay_program_id: [19; ID_BYTES],
            market: [2; ID_BYTES],
            generation: 7,
            relayer_key_set: [14; ID_BYTES],
            relayer_key_set_staging_vacancy: [15; ID_BYTES],
            compute_unit_limit: Some(2),
            compute_unit_price_micro_lamports: Some(1_000_001),
            address_lookup_table: None,
        }
    }

    #[test]
    fn canonical_create_frame_has_one_owner_and_refuses_alias_substitution() {
        let keys = frame_keys();
        let ordered = ordered_frame_keys(&keys).expect("canonical frame");
        assert_eq!(ordered.len(), 21);
        let roles = relay_frame_roles_v1(RelayFrameKindV1::CreateRecord);
        assert_eq!(roles.len(), ordered.len());
        assert!(roles[0].is_signer());
        assert!(roles[0].is_writable());
        assert!(roles[4].is_writable());
        assert_eq!(roles.iter().filter(|role| role.is_writable()).count(), 2);

        let mut aliased = keys;
        aliased.adapter.raw = aliased.material.raw;
        assert!(ordered_frame_keys(&aliased).is_err());
    }

    #[test]
    fn finalized_record_pair_binds_body_address_and_staging_vacancy() {
        let registry = [70; ID_BYTES];
        let schema = [71; ID_BYTES];
        let body = vec![1, 2, 3, 4];
        let digest: [u8; ID_BYTES] = Sha256::digest(&body).into();
        let pair = record_pair(registry, schema, digest);
        let raw = account(registry, 0, body.clone());
        authenticate_record_pair(
            registry,
            pair,
            schema,
            digest,
            &raw,
            None,
            &Rent::free(),
            body.len(),
        )
        .expect("finalized pair");

        let substituted = account(registry, 0, vec![1, 2, 3, 5]);
        assert!(
            authenticate_record_pair(
                registry,
                pair,
                schema,
                digest,
                &substituted,
                None,
                &Rent::free(),
                body.len(),
            )
            .is_err()
        );
        let occupied_stage = account(registry, 1, vec![9]);
        assert!(
            authenticate_record_pair(
                registry,
                pair,
                schema,
                digest,
                &raw,
                Some(&occupied_stage),
                &Rent::free(),
                body.len(),
            )
            .is_err()
        );
    }

    #[test]
    fn prestate_digest_ignores_observation_slot_but_binds_every_account() {
        let keys = vec![[1; ID_BYTES], [2; ID_BYTES]];
        let accounts = vec![Some(account([3; ID_BYTES], 4, vec![5, 6])), None];
        let first = FrameSnapshotV1 {
            slot: 10,
            keys: keys.clone(),
            accounts: accounts.clone(),
        };
        let later = FrameSnapshotV1 {
            slot: 99,
            keys,
            accounts: accounts.clone(),
        };
        assert_eq!(prestate_digest(&first), prestate_digest(&later));
        let mut substituted = accounts;
        substituted[0].as_mut().expect("account").lamports = 5;
        let changed = FrameSnapshotV1 {
            slot: 99,
            keys: later.keys,
            accounts: substituted,
        };
        assert_ne!(prestate_digest(&first), prestate_digest(&changed));
    }

    #[test]
    fn exact_collecting_resume_refuses_a_substituted_binding() {
        let keys = frame_keys();
        let artifact = ArtifactRouteV1 {
            set_name: "one".to_owned(),
            account_set_id: [30; ID_BYTES],
            observed_slot: 44,
            set_count: 1,
            decoding_rules_id: [31; ID_BYTES],
            signer: [32; ID_BYTES],
        };
        let generation = 7;
        let material = [33; ID_BYTES];
        let provider = [34; ID_BYTES];
        let key_set = [35; ID_BYTES];
        let mut seed = [0; SET_DIGEST_SEED_PREIMAGE_BYTES];
        encode_set_digest_seed_preimage_v1(
            &mut seed,
            artifact.account_set_id,
            artifact.observed_slot,
        )
        .expect("seed");
        let seed_digest = Sha256::digest(seed).into();
        let mut bytes = vec![0; relayed_observation_record_bytes_v1(1).expect("width")];
        create_relayed_observation_record_into_v1(
            &mut bytes,
            RelayedRecordBindingV1 {
                market: keys.market,
                generation,
                source_material_id: material,
                account_set_id: artifact.account_set_id,
                provider_release_id: provider,
                relayer_key_set_id: key_set,
                observed_cluster_id: SOLANA_MAINNET_GENESIS_HASH_V1,
                observed_slot: artifact.observed_slot,
            },
            keys.beneficiary,
            1,
            1,
            seed_digest,
            50,
        )
        .expect("record");
        let record = account([19; ID_BYTES], 100, bytes.clone());
        let mut clock_data = vec![0; crate::chain::CLOCK_ACCOUNT_BYTES];
        clock_data
            .get_mut(
                crate::chain::CLOCK_UNIX_TIMESTAMP_OFFSET
                    ..crate::chain::CLOCK_UNIX_TIMESTAMP_OFFSET + 8,
            )
            .expect("timestamp")
            .copy_from_slice(&60_i64.to_le_bytes());
        let clock = account(
            parse_id32("sysvar", SYSVAR_PROGRAM_ID_BASE58).expect("sysvar"),
            1,
            clock_data,
        );
        assert_eq!(
            authenticate_record_prestate(
                Some(&record),
                [19; ID_BYTES],
                &keys,
                &artifact,
                generation,
                material,
                provider,
                key_set,
                1,
                &clock,
            )
            .expect("exact resume"),
            (100, true)
        );
        assert!(
            authenticate_record_prestate(
                Some(&record),
                [19; ID_BYTES],
                &keys,
                &artifact,
                generation,
                [99; ID_BYTES],
                provider,
                key_set,
                1,
                &clock,
            )
            .is_err()
        );
    }

    fn resume_message(
        account_set_id: [u8; ID_BYTES],
        observed_slot: u64,
        set_index: u16,
        set_count: u16,
        byte: u8,
    ) -> Vec<u8> {
        let inline = [byte; 4];
        let body = AccountObservationV1::new(
            [byte; ID_BYTES],
            [byte.saturating_add(20); ID_BYTES],
            u64::from(byte),
            4,
            &inline,
            false,
            SHA256_EMPTY_DIGEST,
        )
        .expect("body");
        let message = AttestationMessageV1::new(
            SOLANA_MAINNET_GENESIS_HASH_V1,
            RELAYED_FAMILY_RELEASE_ID_V1,
            [88; ID_BYTES],
            account_set_id,
            observed_slot,
            set_index,
            set_count,
            body,
        )
        .expect("message");
        let mut bytes = vec![0; message.encoded_len()];
        message.encode_into(&mut bytes).expect("message bytes");
        bytes
    }

    #[test]
    fn append_resume_authenticates_prefix_then_stops_only_at_exact_sealed() {
        let relay = [19; ID_BYTES];
        let market = [20; ID_BYTES];
        let generation = 7;
        let account_set_id = [21; ID_BYTES];
        let observed_slot = 44;
        let material = [22; ID_BYTES];
        let provider = [23; ID_BYTES];
        let key_set = [24; ID_BYTES];
        let beneficiary = [25; ID_BYTES];
        let messages = vec![
            resume_message(account_set_id, observed_slot, 0, 2, 1),
            resume_message(account_set_id, observed_slot, 1, 2, 2),
        ];
        let message_refs: Vec<&[u8]> = messages.iter().map(Vec::as_slice).collect();
        let mut fold =
            crate::derive::SetDigestFold::seed(account_set_id, observed_slot).expect("fold seed");
        let mut folded = Vec::new();
        for message in &messages {
            fold.absorb(
                message
                    .get(RELAYED_ATTESTATION_HEAD_BYTES..)
                    .expect("body bytes"),
            );
            folded.push(fold.digest());
        }
        let seal = ObservationSetSealV1::new(
            SOLANA_MAINNET_GENESIS_HASH_V1,
            RELAYED_FAMILY_RELEASE_ID_V1,
            account_set_id,
            observed_slot,
            2,
            fold.digest(),
        )
        .expect("seal");
        let seal_bytes = seal.to_bytes().expect("seal bytes");
        let binding = RelayedRecordBindingV1 {
            market,
            generation,
            source_material_id: material,
            account_set_id,
            provider_release_id: provider,
            relayer_key_set_id: key_set,
            observed_cluster_id: SOLANA_MAINNET_GENESIS_HASH_V1,
            observed_slot,
        };
        let mut bytes = vec![0; relayed_observation_record_bytes_v1(2).expect("width")];
        let initial = crate::derive::SetDigestFold::seed(account_set_id, observed_slot)
            .expect("seed")
            .digest();
        create_relayed_observation_record_into_v1(
            &mut bytes,
            binding,
            beneficiary,
            2,
            1,
            initial,
            50,
        )
        .expect("record");
        let record_address = crate::txn::derive_record_address(
            relay,
            market,
            generation,
            account_set_id,
            observed_slot,
        )
        .0;
        let as_account = |data: Vec<u8>| account(relay, 100, data);
        assert_eq!(
            inspect_record_resume_v1(
                relay,
                record_address,
                market,
                generation,
                account_set_id,
                observed_slot,
                &as_account(bytes.clone()),
                &message_refs,
                &seal_bytes,
            )
            .expect("empty resume"),
            RecordResumeV1::AppendFrom(0)
        );

        let first = AttestationMessageV1::decode(messages.first().expect("first message"))
            .expect("first decode");
        append_relayed_observation_in_place_v1(
            &mut bytes,
            binding,
            first,
            *folded.first().expect("first fold"),
        )
        .expect("first append");
        assert_eq!(
            inspect_record_resume_v1(
                relay,
                record_address,
                market,
                generation,
                account_set_id,
                observed_slot,
                &as_account(bytes.clone()),
                &message_refs,
                &seal_bytes,
            )
            .expect("prefix resume"),
            RecordResumeV1::AppendFrom(1)
        );

        let divergent = [
            resume_message(account_set_id, observed_slot, 0, 2, 9),
            messages.get(1).expect("second message").clone(),
        ];
        let divergent_refs: Vec<&[u8]> = divergent.iter().map(Vec::as_slice).collect();
        assert!(
            inspect_record_resume_v1(
                relay,
                record_address,
                market,
                generation,
                account_set_id,
                observed_slot,
                &as_account(bytes.clone()),
                &divergent_refs,
                &seal_bytes,
            )
            .is_err()
        );

        let second = AttestationMessageV1::decode(messages.get(1).expect("second message"))
            .expect("second decode");
        append_relayed_observation_in_place_v1(
            &mut bytes,
            binding,
            second,
            *folded.get(1).expect("second fold"),
        )
        .expect("second append");
        assert_eq!(
            inspect_record_resume_v1(
                relay,
                record_address,
                market,
                generation,
                account_set_id,
                observed_slot,
                &as_account(bytes.clone()),
                &message_refs,
                &seal_bytes,
            )
            .expect("all-appended resume"),
            RecordResumeV1::AppendFrom(2)
        );
        assert!(
            inspect_record_resume_v1(
                relay,
                record_address,
                market,
                generation,
                account_set_id,
                observed_slot,
                &as_account(bytes.clone()),
                message_refs.get(..1).expect("short artifact"),
                &seal_bytes,
            )
            .is_err(),
            "a record filled past the supplied artifact must refuse"
        );
        seal_relayed_observation_in_place_v1(&mut bytes, binding, seal, 0, 60)
            .expect("seal record");
        assert_eq!(
            inspect_record_resume_v1(
                relay,
                record_address,
                market,
                generation,
                account_set_id,
                observed_slot,
                &as_account(bytes),
                &message_refs,
                &seal_bytes,
            )
            .expect("sealed resume"),
            RecordResumeV1::Complete
        );
    }

    #[test]
    fn packet_and_priority_fee_bounds_are_explicit() {
        let submit = submit();
        assert_eq!(priority_fee_ceiling(&submit).expect("priority ceiling"), 3);
        let keys = frame_keys();
        let addresses = ordered_frame_keys(&keys).expect("frame");
        let roles = relay_frame_roles_v1(RelayFrameKindV1::CreateRecord);
        let instruction = Instruction {
            program_id: Address::from(submit.relay_program_id),
            accounts: addresses
                .into_iter()
                .zip(roles)
                .map(|(key, role)| AccountMeta {
                    pubkey: Address::from(key),
                    is_signer: role.is_signer(),
                    is_writable: role.is_writable(),
                })
                .collect(),
            data: vec![0; 136],
        };
        let bound = packet_bound(&submit, keys.worker, &instruction).expect("packet bound");
        assert_eq!(bound.selected_message_version, "legacy");
        assert_eq!(bound.v0_packet_bytes, 0);
        assert!(bound.legacy_packet_bytes <= SOLANA_PACKET_DATA_BYTES);

        let mut routed_submit = submit;
        routed_submit.address_lookup_table = Some(crate::config::AddressLookupTableConfig {
            key: [90; ID_BYTES],
            addresses: instruction
                .accounts
                .iter()
                .filter(|meta| !meta.is_signer)
                .map(|meta| meta.pubkey.to_bytes())
                .collect(),
        });
        let routed = packet_bound(&routed_submit, keys.worker, &instruction).expect("v0 bound");
        assert_eq!(routed.selected_message_version, "v0");
        assert!(routed.v0_packet_bytes > 0);
        assert!(routed.lookup_table_addresses > 0);
        assert!(routed.v0_packet_bytes < routed.legacy_packet_bytes);
    }

    fn signed_artifact_fixture() -> (TempDir, Config) {
        let temp = TempDir::new().expect("temp");
        let position = PositionConfig {
            key: [41; ID_BYTES],
            expected_owner: [42; ID_BYTES],
            inline_len: 4,
            admitted_data_lens: vec![4],
        };
        let account_set_id = [43; ID_BYTES];
        let decoding_rules_id = [44; ID_BYTES];
        let set = AccountSetConfig {
            name: "one".to_owned(),
            relay_family_id: RELAYED_FAMILY_RELEASE_ID_V1,
            decoding_rules_id,
            positions: vec![position.clone()],
            account_set_id,
        };
        let config = Config {
            source_path: temp.path().join("config.toml"),
            output_dir: temp.path().join("out"),
            poll_interval: Duration::from_secs(1),
            body_page_bytes: 1024,
            request_timeout: Duration::from_secs(1),
            rpc_endpoints: vec!["http://127.0.0.1:8899".to_owned()],
            expected_genesis_hash: SOLANA_MAINNET_GENESIS_HASH_V1,
            rehearsal_attested_cluster_id: None,
            attestation_keypair_path: temp.path().join("unused-attestation.json"),
            fee_payer_keypair_path: None,
            submit: None,
            account_sets: vec![set],
        };
        let signing = SigningKey::from_bytes(&[45; ID_BYTES]);
        let signer = signing.verifying_key().to_bytes();
        let body = AccountObservationV1::new(
            position.key,
            position.expected_owner,
            10,
            4,
            &[1, 2, 3, 4],
            false,
            SHA256_EMPTY_DIGEST,
        )
        .expect("body");
        let attestation = AttestationMessageV1::new(
            SOLANA_MAINNET_GENESIS_HASH_V1,
            RELAYED_FAMILY_RELEASE_ID_V1,
            decoding_rules_id,
            account_set_id,
            55,
            0,
            1,
            body,
        )
        .expect("attestation");
        let mut message = vec![0; attestation.encoded_len()];
        attestation.encode_into(&mut message).expect("encode");
        let signature = signing.sign(&message).to_bytes();
        let mut seed = [0; SET_DIGEST_SEED_PREIMAGE_BYTES];
        encode_set_digest_seed_preimage_v1(&mut seed, account_set_id, 55).expect("seed");
        let running_seed: [u8; ID_BYTES] = Sha256::digest(seed).into();
        let mut fold = Sha256::new();
        fold.update(running_seed);
        fold.update(
            message
                .get(RELAYED_ATTESTATION_HEAD_BYTES..)
                .expect("body bytes"),
        );
        let set_digest: [u8; ID_BYTES] = fold.finalize().into();
        let seal = ObservationSetSealV1::new(
            SOLANA_MAINNET_GENESIS_HASH_V1,
            RELAYED_FAMILY_RELEASE_ID_V1,
            account_set_id,
            55,
            1,
            set_digest,
        )
        .expect("seal")
        .to_bytes()
        .expect("seal bytes");
        let seal_signature = signing.sign(&seal).to_bytes();
        std::fs::write(temp.path().join("attestation.0.bin"), &message).expect("message");
        std::fs::write(temp.path().join("attestation.0.sig"), signature).expect("signature");
        std::fs::write(temp.path().join("seal.bin"), seal).expect("seal");
        std::fs::write(temp.path().join("seal.sig"), seal_signature).expect("seal signature");
        let manifest = serde_json::json!({
            "artifact_schema": "dclutch.relayer.dry-run.v1",
            "set_name": "one",
            "observed_cluster_id_hex": to_hex(&SOLANA_MAINNET_GENESIS_HASH_V1),
            "relay_family_id_hex": to_hex(&RELAYED_FAMILY_RELEASE_ID_V1),
            "decoding_rules_id_hex": to_hex(&decoding_rules_id),
            "account_set_id_hex": to_hex(&account_set_id),
            "observed_slot": 55,
            "set_count": 1,
            "set_digest_hex": to_hex(&set_digest),
            "attestation_signer_pubkey_hex": to_hex(&signer),
            "rehearsal_twin": null,
            "positions": [{
                "set_index": 0,
                "message_file": "attestation.0.bin",
                "signature_file": "attestation.0.sig"
            }]
        });
        std::fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).expect("manifest"),
        )
        .expect("manifest write");
        (temp, config)
    }

    #[test]
    fn artifact_producer_authenticates_payloads_and_refuses_substitution() {
        let (temp, config) = signed_artifact_fixture();
        let artifact = read_artifact(temp.path(), &config).expect("signed artifact");
        assert_eq!(artifact.observed_slot, 55);
        let path = temp.path().join("attestation.0.bin");
        let mut substituted = std::fs::read(&path).expect("message");
        substituted.push(0);
        std::fs::write(&path, substituted).expect("substitute");
        assert!(read_artifact(temp.path(), &config).is_err());
    }

    #[test]
    fn rehearsal_artifact_is_never_promoted_to_public_devnet() {
        let (temp, mut config) = signed_artifact_fixture();
        config.rehearsal_attested_cluster_id = Some(SOLANA_MAINNET_GENESIS_HASH_V1);
        assert!(read_artifact(temp.path(), &config).is_err());
    }

    #[test]
    fn keeper_submit_gate_accepts_only_exact_devnet_genesis() {
        let (_temp, mut config) = signed_artifact_fixture();
        config.submit = Some(submit());
        assert!(require_devnet_submit(&config).is_ok());
        config
            .submit
            .as_mut()
            .expect("submit")
            .expected_genesis_hash = SOLANA_MAINNET_GENESIS_HASH_V1;
        assert!(require_devnet_submit(&config).is_err());
    }
}
