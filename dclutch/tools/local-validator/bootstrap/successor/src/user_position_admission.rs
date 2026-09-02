//! Exterior, restart-safe execution of one wallet User Position admission.
//!
//! The transaction body is owned exclusively by
//! `dclutch_operator::user_position_admission_v1`. This module supplies the
//! exterior guarantees that cannot live in an unsigned operator: one bounded
//! finalized devnet observation, durable intent before key access, exact fee
//! accounting, finalized-only submission evidence, and hostile poststate.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims_svm::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketViewV2,
        LiabilityBasisPositionInputV2, LiabilityBasisPositionViewV2,
        encode_liability_basis_position_into_v2, liability_basis_vector_width_v2,
    },
    protocol_position_v2::ProtocolPositionAdmissionV2,
};
use dclutch_custody_contract::CustodyAuthoritySeedsV1;
use dclutch_market_core_codec::{CoreState, Phase as CorePhase, StateBumpsV1};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    user_position_admission_v1::{
        UserPositionAdmissionPlanV1, UserPositionAdmissionSnapshotV1,
        plan_user_position_admission_v1,
    },
};
use dclutch_product_payoff_v2_codec::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
    require_slot_pinned_release_v1, slot_pinned_release_elf_digest_v1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1;
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_token_svm::{
    ACCOUNT_BYTES, AccountState as TokenAccountState, COption, CollateralAdapterReleaseV1, Mint,
    TOKEN_2022_PROGRAM_ID, TokenAccount,
    instruction::{InstructionSpec, approve_checked, initialize_account3, transfer_checked},
};
use dclutch_versioned_message_operator::compile_v0_message_with_optional_tables;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::{Message, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::{Transaction, VersionedTransaction},
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::create_account_with_seed;

use crate::{
    Error, Result, campaign,
    chaos_fault::{self, BoundaryV1},
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1},
    model::SuccessorPlan,
    plan::{hex, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1, parse_json_without_duplicate_keys_v1},
};

const REPORT_SCHEMA_V1: &str = "dclutch-devnet-user-position-admission-execution-v1";
const LOCAL_REPORT_SCHEMA_V1: &str = "dclutch-owned-loopback-user-position-admission-execution-v1";
const FINALITY_WAIT: Duration = Duration::from_secs(300);
const DIRECT_COLLATERAL_SEED_DOMAIN_V1: &[u8] = b"dclutch:direct-collateral:v1";
const POSITION_ADMISSION_CHAOS_MUTATION_V1: &str = "position-admission";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    campaign_evidence: PathBuf,
    position_owner: Pubkey,
    position_owner_keypair: PathBuf,
    fee_payer: Pubkey,
    fee_payer_keypair: PathBuf,
    minimum_finalized_slot: u64,
    output: PathBuf,
    execute: bool,
    collateral: Option<CollateralArgumentsV1>,
    /// Frozen founding routing tables the v0 admission message may load
    /// addresses through. Comma-separated in one `--routing-table` value; the
    /// snapshot fetches them in the same finalized observation as every
    /// semantic account, exactly as the founding ladder does.
    routing_tables: Vec<Pubkey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CollateralArgumentsV1 {
    source_owner: Pubkey,
    source_owner_keypair: PathBuf,
    source_account: Pubkey,
    quantity_atoms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PhaseV1 {
    Planned,
    SignedNotSubmitted,
    Dispatching,
    Submitted,
    Finalized,
}

/// One observation of the durable admission packet's exact signature.
///
/// `Absent` is deliberately distinct from a present but not-yet-finalized
/// signature: only the former can authorize resending the already-signed
/// packet from `Dispatching`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionSignatureStateV1 {
    Absent,
    Pending,
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionRecoveryV1 {
    SignOnce,
    ResendIdenticalPacket,
    PollOnly,
    Finalize,
    Complete,
    RefuseHistoricAmbiguity,
}

fn admission_recovery_v1(
    phase: PhaseV1,
    signature: Option<AdmissionSignatureStateV1>,
) -> Result<AdmissionRecoveryV1> {
    let recovery = match (phase, signature) {
        (PhaseV1::Planned, None) => AdmissionRecoveryV1::SignOnce,
        (PhaseV1::Dispatching, Some(AdmissionSignatureStateV1::Absent)) => {
            AdmissionRecoveryV1::ResendIdenticalPacket
        }
        (
            PhaseV1::Dispatching | PhaseV1::Submitted | PhaseV1::SignedNotSubmitted,
            Some(AdmissionSignatureStateV1::Pending),
        ) => AdmissionRecoveryV1::PollOnly,
        (
            PhaseV1::Dispatching | PhaseV1::Submitted | PhaseV1::SignedNotSubmitted,
            Some(AdmissionSignatureStateV1::Finalized),
        ) => AdmissionRecoveryV1::Finalize,
        (
            PhaseV1::Submitted | PhaseV1::SignedNotSubmitted,
            Some(AdmissionSignatureStateV1::Absent),
        ) => AdmissionRecoveryV1::RefuseHistoricAmbiguity,
        (PhaseV1::Finalized, None) => AdmissionRecoveryV1::Complete,
        _ => {
            return Err(Error::new(
                "admission phase and exact signature observation are inconsistent",
            ));
        }
    };
    Ok(recovery)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum CollateralPhaseV1 {
    Planned,
    SignedNotSubmitted,
    Submitted,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountStateV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    rent_epoch: u64,
    data_len: usize,
    data_sha256: String,
    account_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionEvidenceV1 {
    program_id: String,
    accounts: Vec<InstructionAccountV1>,
    data_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntentV1 {
    plan_sha256: String,
    campaign_evidence_sha256: String,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    minimum_finalized_slot: u64,
    position_owner: String,
    fee_payer: String,
    claims_market: String,
    position: String,
    admission: String,
    founding_trading_custody_replay: String,
    position_rent_principal_lamports: u64,
    admission_rent_principal_lamports: u64,
    position_top_up_lamports: u64,
    admission_top_up_lamports: u64,
    transaction_fee_lamports: u64,
    total_owner_debit_lamports: u64,
    total_fee_payer_debit_lamports: u64,
    recent_blockhash: String,
    last_valid_block_height: u64,
    wire_bytes: usize,
    message_base64: String,
    message_sha256: String,
    claims_request_sha256: String,
    expected_receipt_producer: String,
    expected_receipt_base64: String,
    expected_position_base64: String,
    expected_admission_base64: String,
    instructions: Vec<InstructionEvidenceV1>,
    prestate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FinalizedEvidenceV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    return_data_producer: String,
    return_data_sha256: String,
    poststate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AdmissionHistoryV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    return_data_producer: String,
    return_data_sha256: String,
    poststate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollateralIntentV1 {
    admission_intent_sha256: String,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    minimum_finalized_slot: u64,
    market: String,
    registry_program: String,
    realm_record: String,
    realm_data_sha256: String,
    collateral_adapter_release: String,
    release_set: String,
    activation_cache: String,
    custody_program: String,
    custody_programdata: String,
    custody_artifact_release: String,
    custody_authority: String,
    token_program: String,
    mint: String,
    mint_decimals: u8,
    participant: String,
    participant_token_seed: String,
    participant_token_account: String,
    source_owner: String,
    source_account: String,
    quantity_atoms: u64,
    creates_participant_account: bool,
    participant_account_rent_lamports: u64,
    transaction_fee_lamports: u64,
    total_fee_payer_debit_lamports: u64,
    fee_payer: String,
    recent_blockhash: String,
    last_valid_block_height: u64,
    wire_bytes: usize,
    message_base64: String,
    message_sha256: String,
    expected_return_data: Option<String>,
    market_pre_base64: String,
    realm_pre_base64: String,
    registry_program_pre_base64: String,
    activation_cache_pre_base64: String,
    custody_program_pre_base64: String,
    custody_programdata_pre_base64: String,
    mint_pre_base64: String,
    source_pre_base64: String,
    participant_pre_base64: Option<String>,
    participant_transfer_pre_base64: String,
    expected_source_base64: String,
    expected_participant_token_base64: String,
    instructions: Vec<InstructionEvidenceV1>,
    prestate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollateralFinalizedEvidenceV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    return_data: Option<String>,
    poststate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CollateralHistoryV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    return_data: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollateralReportV1 {
    phase: CollateralPhaseV1,
    intent_sha256: String,
    envelope_sha256: String,
    intent: CollateralIntentV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalized: Option<CollateralFinalizedEvidenceV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportV1 {
    schema: String,
    cluster: String,
    rpc_url: String,
    authorized_mutation: bool,
    phase: PhaseV1,
    intent_sha256: String,
    envelope_sha256: String,
    intent: IntentV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalized: Option<FinalizedEvidenceV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    collateral: Option<CollateralReportV1>,
}

/// Finalized participant facts admitted for one later Direct session.
///
/// This is deliberately a projection of [`ReportV1`], not another persisted
/// report shape.  The admission exterior remains the sole owner of its JSON
/// and transaction-history joins; Direct receives only the coordinates it can
/// independently reauthenticate against its own finalized snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedDirectParticipantEvidenceV1 {
    pub(crate) market: Pubkey,
    pub(crate) claims_market: Pubkey,
    pub(crate) position: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) collateral_account: Pubkey,
    pub(crate) collateral_quantity_atoms: u64,
    pub(crate) custody_authority: Pubkey,
    pub(crate) mint: Pubkey,
    pub(crate) token_program: Pubkey,
    pub(crate) admission_signature: String,
    pub(crate) admission_slot: u64,
    pub(crate) collateral_signature: String,
    pub(crate) collateral_slot: u64,
}

/// Decode the exact owned-loopback participant report and reopen both of its
/// finalized transactions before exposing any Direct-facing coordinate.
pub(crate) fn parse_finalized_direct_participant_evidence_v1(
    bytes: &[u8],
    rpc: &mut Rpc,
) -> Result<FinalizedDirectParticipantEvidenceV1> {
    parse_finalized_direct_participant_evidence_for_cluster_v1(
        bytes,
        rpc,
        ExpectedClusterV1::OwnedLoopback,
    )
}

/// Reopen both finalized participant transactions for the already-admitted
/// cluster before exposing one Direct-facing projection. Public devnet and an
/// owned loopback use the same report/history semantic owner; only their
/// schema, origin, genesis, and fee policy differ.
pub(crate) fn parse_finalized_direct_participant_evidence_for_cluster_v1(
    bytes: &[u8],
    rpc: &mut Rpc,
    expected_cluster: ExpectedClusterV1,
) -> Result<FinalizedDirectParticipantEvidenceV1> {
    let projected =
        parse_finalized_direct_participant_evidence_offline_v1(bytes, expected_cluster)?;
    let value = parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("Direct participant evidence {error}")))?;
    let report: ReportV1 = serde_json::from_value(value)
        .map_err(|error| Error::new(format!("Direct participant evidence shape: {error}")))?;
    verify_persisted_admission_history(rpc, &report)?;
    verify_persisted_collateral(rpc, &report)?;
    Ok(projected)
}

/// Hostile-decode one finalized participant report without opening a wallet or
/// contacting RPC. This is the producer-side half of the trust boundary: it
/// authenticates the report's exact persisted envelopes and projects the
/// existing Direct evidence type. The Direct executor still reopens current
/// finalized account state before it can build or submit anything.
pub(crate) fn parse_finalized_direct_participant_evidence_offline_v1(
    bytes: &[u8],
    expected_cluster: ExpectedClusterV1,
) -> Result<FinalizedDirectParticipantEvidenceV1> {
    let value = parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("Direct participant evidence {error}")))?;
    let report: ReportV1 = serde_json::from_value(value.clone())
        .map_err(|error| Error::new(format!("Direct participant evidence shape: {error}")))?;
    if serde_json::to_value(&report)? != value {
        return Err(Error::new(
            "Direct participant evidence contained an unknown, defaulted, or noncanonical field",
        ));
    }
    authenticate_report_phase_envelopes(&report)?;
    authenticate_intent_digest(&report)?;
    let collateral = report.collateral.as_ref().ok_or_else(|| {
        Error::new("Direct participant evidence omitted finalized collateral preparation")
    })?;
    authenticate_collateral_intent_digest(collateral)?;
    if report.schema != report_schema_v1(expected_cluster)
        || report.cluster != expected_cluster.evidence_label()
        || !report.authorized_mutation
        || report.phase != PhaseV1::Finalized
        || collateral.phase != CollateralPhaseV1::Finalized
    {
        return Err(Error::new(
            "Direct participant evidence was not one finalized authorized report for the selected cluster",
        ));
    }
    project_finalized_direct_participant_evidence_v1(&report)
}

fn project_finalized_direct_participant_evidence_v1(
    report: &ReportV1,
) -> Result<FinalizedDirectParticipantEvidenceV1> {
    let admission = report
        .finalized
        .as_ref()
        .ok_or_else(|| Error::new("Direct participant admission omitted finalized evidence"))?;
    let collateral = report
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("Direct participant evidence omitted collateral preparation"))?;
    let collateral_finalized = collateral
        .finalized
        .as_ref()
        .ok_or_else(|| Error::new("Direct participant collateral omitted finalized evidence"))?;
    if report.intent.position_owner != collateral.intent.participant
        || collateral.intent.quantity_atoms == 0
    {
        return Err(Error::new(
            "Direct participant owner or collateral quantity changed across finalized legs",
        ));
    }
    Ok(FinalizedDirectParticipantEvidenceV1 {
        market: Pubkey::from_str(&collateral.intent.market)
            .map_err(|error| Error::new(format!("Direct participant Market: {error}")))?,
        claims_market: Pubkey::from_str(&report.intent.claims_market)
            .map_err(|error| Error::new(format!("Direct participant Claims aggregate: {error}")))?,
        position: Pubkey::from_str(&report.intent.position)
            .map_err(|error| Error::new(format!("Direct participant Position: {error}")))?,
        owner: Pubkey::from_str(&report.intent.position_owner)
            .map_err(|error| Error::new(format!("Direct participant owner: {error}")))?,
        collateral_account: Pubkey::from_str(&collateral.intent.participant_token_account)
            .map_err(|error| {
                Error::new(format!("Direct participant collateral account: {error}"))
            })?,
        collateral_quantity_atoms: collateral.intent.quantity_atoms,
        custody_authority: Pubkey::from_str(&collateral.intent.custody_authority).map_err(
            |error| Error::new(format!("Direct participant Custody authority: {error}")),
        )?,
        mint: Pubkey::from_str(&collateral.intent.mint)
            .map_err(|error| Error::new(format!("Direct participant Mint: {error}")))?,
        token_program: Pubkey::from_str(&collateral.intent.token_program)
            .map_err(|error| Error::new(format!("Direct participant token program: {error}")))?,
        admission_signature: admission.signature.clone(),
        admission_slot: admission.slot,
        collateral_signature: collateral_finalized.signature.clone(),
        collateral_slot: collateral_finalized.slot,
    })
}

#[derive(Clone, Copy)]
struct CoordinatesV1 {
    claims_market: Pubkey,
    core_market: Pubkey,
    rent_credit: Pubkey,
    trading_replay: Pubkey,
    product: Pubkey,
    result_domain: Pubkey,
    portfolio: Pubkey,
    linked_basis: Pubkey,
}

#[derive(Clone, Copy)]
struct RecordCoordinatesV1 {
    raw: Pubkey,
    staging: Pubkey,
}

#[derive(Clone)]
struct SnapshotBundleV1 {
    operator: UserPositionAdmissionSnapshotV1,
    replay: ObservedAccount,
    fee_payer: ObservedAccount,
    states: BTreeMap<String, AccountStateV1>,
    /// Caller-selected frozen routing tables, observed in the same finalized
    /// snapshot as every semantic account so the v0 compiler admits them.
    routing_tables: Vec<ObservedAccount>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndependentCollateralCoordinatesV1 {
    market: String,
    realm_record: String,
    mint: String,
    registry_program: String,
    release_set: String,
    core_program: String,
    custody_program: String,
    custody_programdata: String,
    custody_artifact_release: String,
    realm_data_sha256: String,
    mint_data_sha256: String,
    custody_programdata_sha256: String,
}

struct CustodyActivationSnapshotV1<'a> {
    registry_program: Pubkey,
    release_set: [u8; 32],
    activation_cache: Pubkey,
    activation_cache_account: &'a RpcAccount,
    custody_program: Pubkey,
    custody_program_account: &'a RpcAccount,
    custody_programdata: Pubkey,
    custody_programdata_account: &'a RpcAccount,
}

struct ReportJournalV1 {
    path: PathBuf,
    expected_bytes: Option<Vec<u8>>,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    run_with_expected_cluster(arguments, ExpectedClusterV1::Devnet)
}

pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    run_with_expected_cluster(arguments, ExpectedClusterV1::OwnedLoopback)
}

fn run_with_expected_cluster(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    expected_cluster.authenticate(&arguments.origin)?;
    let plan_source = fs::read(&arguments.plan)?;
    let plan_sha256 = sha256_hex(&plan_source);
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let evidence_source = fs::read(&arguments.campaign_evidence)?;
    let evidence_sha256 = sha256_hex(&evidence_source);
    let evidence: Value = serde_json::from_slice(&evidence_source)?;
    authenticate_plan_evidence(&plan_sha256, &evidence)?;
    let coordinates = evidence_coordinates(&evidence)?;

    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    authenticate_genesis_again(&mut rpc, &arguments.origin)?;

    if arguments.output.exists() {
        let report_bytes = fs::read(&arguments.output)?;
        let mut report: ReportV1 = serde_json::from_slice(&report_bytes)?;
        let mut journal = ReportJournalV1::existing(arguments.output.clone(), report_bytes);
        authenticate_existing_report(
            &report,
            &arguments,
            &plan_sha256,
            &evidence_sha256,
            &plan,
            &evidence,
            expected_cluster,
        )?;
        if arguments.execute && !report.authorized_mutation {
            // A read-only planning invocation may be resumed later under a
            // fresh explicit --execute authorization. Record that expansion
            // durably before the first key-file read.
            report.authorized_mutation = true;
            journal.persist(&mut report)?;
        }
        resume_admission_and_collateral(
            &mut rpc,
            &arguments,
            &plan,
            &evidence,
            &mut report,
            &mut journal,
        )?;
        return print_report(&report);
    }

    let snapshot = acquire_snapshot(&mut rpc, &arguments, &plan, coordinates, &evidence)?;
    let unsigned = plan_user_position_admission_v1(&snapshot.operator)
        .map_err(|error| Error::new(format!("User Position admission plan refused: {error:?}")))?;
    // Before the prefund, because the prefund cannot fix this and spends real
    // lamports finding that out.
    require_fee_payer_never_declared_readonly_v1(&unsigned.instructions, arguments.fee_payer)?;
    // The admission frame requires the owner to sign READONLY, and a System
    // transfer debiting the owner in the same transaction forces message-level
    // owner writability - the founding pays its allocated accounts in a
    // separate transaction for exactly this reason. When the Position or
    // admission still owes rent, pay it first in its own finalized transfer,
    // then re-snapshot: the plan then carries zero top-ups and a
    // single-instruction message the frame authenticates.
    let (snapshot, unsigned) = if arguments.execute
        && unsigned
            .position_top_up_lamports
            .checked_add(unsigned.admission_top_up_lamports)
            .ok_or_else(|| Error::new("admission top-up overflow"))?
            != 0
    {
        prefund_admission_rents_v1(&mut rpc, &arguments, &unsigned)?;
        let snapshot = acquire_snapshot(&mut rpc, &arguments, &plan, coordinates, &evidence)?;
        let unsigned = plan_user_position_admission_v1(&snapshot.operator).map_err(|error| {
            Error::new(format!(
                "User Position admission plan refused after prefund: {error:?}"
            ))
        })?;
        if unsigned
            .position_top_up_lamports
            .checked_add(unsigned.admission_top_up_lamports)
            .ok_or_else(|| Error::new("admission top-up overflow"))?
            != 0
        {
            return Err(Error::new(
                "admission rents still owed after the finalized prefund transfer",
            ));
        }
        (snapshot, unsigned)
    } else {
        (snapshot, unsigned)
    };
    let mut report = build_report(
        &mut rpc,
        &arguments,
        &plan_sha256,
        &evidence_sha256,
        &snapshot,
        &unsigned,
        expected_cluster,
    )?;
    let mut journal = ReportJournalV1::vacant(arguments.output.clone());
    journal.persist(&mut report)?;
    if arguments.execute {
        resume_admission_and_collateral(
            &mut rpc,
            &arguments,
            &plan,
            &evidence,
            &mut report,
            &mut journal,
        )?;
    }
    print_report(&report)
}

fn resume_admission_and_collateral(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &Value,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
) -> Result<()> {
    resume(rpc, arguments, plan, evidence, report, journal)?;
    if report.phase == PhaseV1::Finalized && arguments.collateral.is_some() {
        resume_or_plan_collateral(rpc, arguments, plan, evidence, report, journal)?;
    }
    Ok(())
}

fn resume_or_plan_collateral(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &Value,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
) -> Result<()> {
    let collateral_arguments = arguments
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("collateral arguments disappeared during resume"))?;
    if report.collateral.is_none() {
        let collateral = build_collateral_report_v1(
            rpc,
            arguments,
            collateral_arguments,
            plan,
            evidence,
            report,
        )?;
        report.collateral = Some(collateral);
        // The complete second message, exact raw-atom quantity, rent, fee,
        // source/target bytes, and output path are durable before any new key
        // file is read.
        journal.persist(report)?;
    }
    authenticate_collateral_arguments(report, collateral_arguments, plan, evidence)?;
    if !arguments.execute {
        return Ok(());
    }
    resume_collateral(rpc, arguments, collateral_arguments, report, journal)
}

fn build_collateral_report_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    collateral_arguments: &CollateralArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &Value,
    report: &ReportV1,
) -> Result<CollateralReportV1> {
    let market = evidence_address(evidence, "founding_market")?;
    let realm_record = evidence_address(evidence, "realm_record")?;
    let registry_program = pubkey(&plan.registry.program_id)?;
    let core_program = pubkey(&plan.core.program_id)?;
    let custody_program = pubkey(&plan.custody.program_id)?;
    let custody_programdata = pubkey(&plan.custody.programdata_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    let (identity_slot, identity_values) = rpc.finalized_accounts(
        &[market, realm_record],
        arguments.minimum_finalized_slot.max(
            report
                .finalized
                .as_ref()
                .map(|value| value.slot)
                .unwrap_or(arguments.minimum_finalized_slot),
        ),
    )?;
    let market_account = identity_values[0]
        .as_ref()
        .ok_or_else(|| Error::new("completed campaign omitted its founding Market"))?;
    let realm_account = identity_values[1]
        .as_ref()
        .ok_or_else(|| Error::new("completed campaign omitted its Realm record"))?;
    if market_account.owner != core_program || market_account.executable {
        return Err(Error::new(
            "founding Market owner or executable bit refused",
        ));
    }
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("founding Market state: {error:?}")))?;
    if market_state.phase != CorePhase::Open
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
        || market_state.identity.registry_program.to_bytes() != registry_program.to_bytes()
    {
        return Err(Error::new(
            "campaign Market was not the exact chain-derived Open Market",
        ));
    }
    let realm_digest: [u8; 32] = Sha256::digest(&realm_account.data).into();
    let expected_realm = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            REALM_SCHEMA_RELEASE_ID_V1.as_slice(),
            realm_digest.as_slice(),
        ],
        &registry_program,
    )
    .0;
    if realm_record != expected_realm
        || realm_account.owner != registry_program
        || realm_account.executable
        || market_state.identity.realm_id.to_bytes() != realm_digest
        || evidence_accounts(evidence)?
            .get("realm_record")
            .and_then(|value| value.get("data_sha256"))
            .and_then(Value::as_str)
            != Some(&sha256_hex(&realm_account.data))
    {
        return Err(Error::new(
            "Realm address, owner, bytes, or Market content join refused",
        ));
    }
    let release_set = market_state.identity.selected_release_set.to_bytes();
    if release_set != decode_hex32(&plan.release_set_id, "plan release set")? {
        return Err(Error::new(
            "Open Market selected a release set other than the checked plan",
        ));
    }
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_slice()],
        &registry_program,
    )
    .0;
    let realm = RealmV1::decode(&realm_account.data)
        .map_err(|error| Error::new(format!("Realm: {error:?}")))?;
    let expected_adapter_release: [u8; 32] = Sha256::digest(
        CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer().to_bytes(),
    )
    .into();
    if realm.token_program() != &TOKEN_2022_PROGRAM_ID
        || realm.collateral_adapter_release_id() != &expected_adapter_release
        || realm.mint_authority_policy() != MintAuthorityPolicy::RequireAbsent
        || realm.freeze_authority_policy() != FreezeAuthorityPolicy::RequireAbsent
    {
        return Err(Error::new(
            "Direct participant collateral requires the Realm's exact Token-2022 no-authority profile",
        ));
    }
    let mint = Pubkey::new_from_array(*realm.collateral_mint());
    if evidence_address(evidence, "collateral_mint")? != mint {
        return Err(Error::new(
            "Realm collateral Mint differed from the completed campaign evidence",
        ));
    }
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release_set).as_slices(),
        &custody_program,
    )
    .0;
    let participant_seed =
        participant_collateral_seed_v1(market, arguments.position_owner, release_set);
    let participant_token =
        Pubkey::create_with_seed(&arguments.position_owner, &participant_seed, &token_program)
            .map_err(|error| Error::new(format!("participant token derivation: {error}")))?;
    if participant_token == collateral_arguments.source_account {
        return Err(Error::new(
            "participant token account must differ from its funding source",
        ));
    }

    let keys = [
        market,
        realm_record,
        mint,
        token_program,
        registry_program,
        activation_cache,
        custody_program,
        custody_programdata,
        custody_authority,
        collateral_arguments.source_account,
        participant_token,
        collateral_arguments.source_owner,
        arguments.position_owner,
        arguments.fee_payer,
    ];
    let (slot, values) = rpc.finalized_accounts(&keys, identity_slot)?;
    let value = |index: usize, label: &str| -> Result<&RpcAccount> {
        values[index]
            .as_ref()
            .ok_or_else(|| Error::new(format!("collateral snapshot omitted {label}")))
    };
    if account_state(market, Some(value(0, "Market")?))
        != account_state(market, Some(market_account))
        || account_state(realm_record, Some(value(1, "Realm")?))
            != account_state(realm_record, Some(realm_account))
    {
        return Err(Error::new(
            "Market or Realm moved between finalized collateral reads",
        ));
    }
    let mint_account = value(2, "Mint")?;
    let token_program_account = value(3, "Token-2022 program")?;
    let registry_program_account = value(4, "Registry program")?;
    let activation_cache_account = value(5, "Registry activation cache")?;
    let custody_program_account = value(6, "Custody program")?;
    let custody_programdata_account = value(7, "Custody ProgramData")?;
    let custody_authority_account = values[8].as_ref();
    let source_account = value(9, "collateral source account")?;
    let participant_account = values[10].as_ref();
    let source_owner_account = value(11, "collateral source owner")?;
    let participant_owner_account = value(12, "participant")?;
    let fee_payer_account = value(13, "fee payer")?;
    if !token_program_account.executable
        || registry_program_account.owner != bpf_loader_upgradeable::ID
        || !registry_program_account.executable
        || ProgramV3View::parse(&registry_program_account.data).is_err()
        || custody_authority_account.is_some_and(|account| {
            account.owner != system_program::ID || account.executable || !account.data.is_empty()
        })
    {
        return Err(Error::new(
            "Token-2022 program, Registry program, or derived Custody authority refused",
        ));
    }
    let activation_rent = rpc.minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?;
    if activation_cache_account.owner != registry_program
        || activation_cache_account.executable
        || activation_cache_account.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || activation_cache_account.lamports < activation_rent
    {
        return Err(Error::new(
            "Registry activation cache owner, width, privilege, or rent refused",
        ));
    }
    let custody_artifact_release =
        authenticate_custody_activation_v1(CustodyActivationSnapshotV1 {
            registry_program,
            release_set,
            activation_cache,
            activation_cache_account,
            custody_program,
            custody_program_account,
            custody_programdata,
            custody_programdata_account,
        })?;
    if sha256_hex(&custody_programdata_account.data) != plan.custody.programdata_sha256 {
        return Err(Error::new(
            "current Custody ProgramData differed from the checked deployment plan",
        ));
    }
    for (label, account) in [
        ("source owner", source_owner_account),
        ("participant", participant_owner_account),
        ("fee payer", fee_payer_account),
    ] {
        if account.owner != system_program::ID || account.executable || !account.data.is_empty() {
            return Err(Error::new(format!(
                "{label} must be an existing System-owned data-empty signer"
            )));
        }
    }
    let parsed_mint = Mint::parse(&mint_account.data)
        .map_err(|error| Error::new(format!("Realm Mint: {error:?}")))?;
    let mint_evidence = evidence_accounts(evidence)?
        .get("collateral_mint")
        .ok_or_else(|| Error::new("campaign evidence omitted collateral_mint"))?;
    if mint_account.owner != token_program
        || !parsed_mint.is_initialized
        || !parsed_mint.mint_authority.is_none()
        || !parsed_mint.freeze_authority.is_none()
        || mint_evidence.get("data_sha256").and_then(Value::as_str)
            != Some(&sha256_hex(&mint_account.data))
    {
        return Err(Error::new(
            "Realm Mint did not satisfy its exact immutable Token-2022 campaign profile",
        ));
    }
    let source = TokenAccount::parse(&source_account.data)
        .map_err(|error| Error::new(format!("collateral source: {error:?}")))?;
    if source_account.owner != token_program
        || source.mint != mint.to_bytes()
        || source.owner != collateral_arguments.source_owner.to_bytes()
        || source.state != TokenAccountState::Initialized
        || !source.native_reserve.is_none()
        || source.amount < collateral_arguments.quantity_atoms
    {
        return Err(Error::new(
            "source owner, Mint, state, native profile, or raw-atom quantity refused",
        ));
    }
    let minimum_participant_rent = rpc.minimum_balance(ACCOUNT_BYTES)?;
    let creates_participant_account = participant_account.is_none();
    let initial_participant_bytes = if let Some(account) = participant_account {
        let parsed = TokenAccount::parse(&account.data)
            .map_err(|error| Error::new(format!("participant token account: {error:?}")))?;
        if account.owner != token_program
            || account.lamports < minimum_participant_rent
            || parsed.mint != mint.to_bytes()
            || parsed.owner != arguments.position_owner.to_bytes()
            || parsed.amount != 0
            || parsed.state != TokenAccountState::Initialized
            || !parsed.delegate.is_none()
            || parsed.delegated_amount != 0
            || !parsed.native_reserve.is_none()
            || !parsed.close_authority.is_none()
        {
            return Err(Error::new(
                "existing participant token account was not the exact empty base profile",
            ));
        }
        account.data.clone()
    } else {
        TokenAccount::initialized_base_bytes(mint.to_bytes(), arguments.position_owner.to_bytes())
            .map_err(|error| Error::new(format!("predict initialized token account: {error:?}")))?
            .to_vec()
    };
    let expected_source_amount = source
        .amount
        .checked_sub(collateral_arguments.quantity_atoms)
        .ok_or_else(|| Error::new("source raw-atom subtraction underflow"))?;
    let expected_source =
        TokenAccount::project_amount_poststate(&source_account.data, expected_source_amount)
            .map_err(|error| Error::new(format!("predict source token poststate: {error:?}")))?;
    let expected_participant = TokenAccount::project_delegated_source_poststate(
        &initial_participant_bytes,
        collateral_arguments.quantity_atoms,
        COption::Some(custody_authority.to_bytes()),
        collateral_arguments.quantity_atoms,
    )
    .map_err(|error| Error::new(format!("predict participant token poststate: {error:?}")))?;

    let participant_rent = if creates_participant_account {
        minimum_participant_rent
    } else {
        0
    };
    let mut instructions = Vec::new();
    if creates_participant_account {
        instructions.push(create_account_with_seed(
            &arguments.fee_payer,
            &participant_token,
            &arguments.position_owner,
            &participant_seed,
            participant_rent,
            u64::try_from(ACCOUNT_BYTES).map_err(|_| Error::new("token width overflow"))?,
            &token_program,
        ));
        instructions.push(token_instruction(
            initialize_account3(
                token_program.to_bytes(),
                participant_token.to_bytes(),
                mint.to_bytes(),
                arguments.position_owner.to_bytes(),
            )
            .map_err(|error| Error::new(format!("InitializeAccount3: {error:?}")))?,
        ));
    }
    instructions.push(token_instruction(
        transfer_checked(
            token_program.to_bytes(),
            collateral_arguments.source_account.to_bytes(),
            mint.to_bytes(),
            participant_token.to_bytes(),
            collateral_arguments.source_owner.to_bytes(),
            collateral_arguments.quantity_atoms,
            parsed_mint.decimals,
        )
        .map_err(|error| Error::new(format!("TransferChecked: {error:?}")))?,
    ));
    instructions.push(token_instruction(
        approve_checked(
            token_program.to_bytes(),
            participant_token.to_bytes(),
            mint.to_bytes(),
            custody_authority.to_bytes(),
            arguments.position_owner.to_bytes(),
            collateral_arguments.quantity_atoms,
            parsed_mint.decimals,
        )
        .map_err(|error| Error::new(format!("ApproveChecked: {error:?}")))?,
    ));
    authenticate_collateral_instruction_sequence_v1(
        &instructions,
        creates_participant_account,
        arguments.fee_payer,
        arguments.position_owner,
        participant_token,
        &participant_seed,
        collateral_arguments,
        token_program,
        mint,
        custody_authority,
        parsed_mint.decimals,
        participant_rent,
    )?;

    let (recent_blockhash, last_valid_block_height) = latest_blockhash(rpc)?;
    let observation = collateral_observation_v1(slot, rpc.block_time(slot)?);
    let compiled = compile_v0_message_with_optional_tables(
        arguments.fee_payer,
        &instructions,
        recent_blockhash,
        observation,
        &[],
    )
    .map_err(|error| Error::new(format!("collateral message compilation: {error:?}")))?;
    let message_bytes = compiled.message.serialize();
    let message_base64 = BASE64.encode(&message_bytes);
    let fee = fee_for_message(rpc, &message_base64)?;
    let total_fee_payer_debit_lamports = participant_rent
        .checked_add(fee)
        .ok_or_else(|| Error::new("collateral rent+fee overflow"))?;
    if fee_payer_account.lamports < total_fee_payer_debit_lamports {
        return Err(Error::new(format!(
            "fee payer has {} lamports but collateral account rent+fee is {}",
            fee_payer_account.lamports, total_fee_payer_debit_lamports
        )));
    }
    let mut prestate = BTreeMap::new();
    for (label, key, account) in [
        ("market", market, Some(market_account)),
        ("realm_record", realm_record, Some(realm_account)),
        ("mint", mint, Some(mint_account)),
        ("token_program", token_program, Some(token_program_account)),
        (
            "registry_program",
            registry_program,
            Some(registry_program_account),
        ),
        (
            "activation_cache",
            activation_cache,
            Some(activation_cache_account),
        ),
        (
            "custody_program",
            custody_program,
            Some(custody_program_account),
        ),
        (
            "custody_programdata",
            custody_programdata,
            Some(custody_programdata_account),
        ),
        (
            "custody_authority",
            custody_authority,
            custody_authority_account,
        ),
        (
            "source_account",
            collateral_arguments.source_account,
            Some(source_account),
        ),
        (
            "participant_token_account",
            participant_token,
            participant_account,
        ),
        (
            "source_owner",
            collateral_arguments.source_owner,
            Some(source_owner_account),
        ),
        (
            "participant",
            arguments.position_owner,
            Some(participant_owner_account),
        ),
        ("fee_payer", arguments.fee_payer, Some(fee_payer_account)),
    ] {
        prestate.insert(label.into(), account_state(key, account));
    }
    let instructions_evidence = instructions
        .iter()
        .map(instruction_evidence)
        .collect::<Vec<_>>();
    let intent = CollateralIntentV1 {
        admission_intent_sha256: report.intent_sha256.clone(),
        observation_slot: slot,
        observation_unix_timestamp: report.intent.observation_unix_timestamp,
        minimum_finalized_slot: arguments.minimum_finalized_slot,
        market: market.to_string(),
        registry_program: registry_program.to_string(),
        realm_record: realm_record.to_string(),
        realm_data_sha256: sha256_hex(&realm_account.data),
        collateral_adapter_release: hex(&expected_adapter_release),
        release_set: hex(&release_set),
        activation_cache: activation_cache.to_string(),
        custody_program: custody_program.to_string(),
        custody_programdata: custody_programdata.to_string(),
        custody_artifact_release,
        custody_authority: custody_authority.to_string(),
        token_program: token_program.to_string(),
        mint: mint.to_string(),
        mint_decimals: parsed_mint.decimals,
        participant: arguments.position_owner.to_string(),
        participant_token_seed: participant_seed,
        participant_token_account: participant_token.to_string(),
        source_owner: collateral_arguments.source_owner.to_string(),
        source_account: collateral_arguments.source_account.to_string(),
        quantity_atoms: collateral_arguments.quantity_atoms,
        creates_participant_account,
        participant_account_rent_lamports: participant_rent,
        transaction_fee_lamports: fee,
        total_fee_payer_debit_lamports,
        fee_payer: arguments.fee_payer.to_string(),
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        wire_bytes: compiled.wire_bytes,
        message_sha256: sha256_hex(&message_bytes),
        message_base64,
        expected_return_data: None,
        market_pre_base64: BASE64.encode(&market_account.data),
        realm_pre_base64: BASE64.encode(&realm_account.data),
        registry_program_pre_base64: BASE64.encode(&registry_program_account.data),
        activation_cache_pre_base64: BASE64.encode(&activation_cache_account.data),
        custody_program_pre_base64: BASE64.encode(&custody_program_account.data),
        custody_programdata_pre_base64: BASE64.encode(&custody_programdata_account.data),
        mint_pre_base64: BASE64.encode(&mint_account.data),
        source_pre_base64: BASE64.encode(&source_account.data),
        participant_pre_base64: participant_account.map(|account| BASE64.encode(&account.data)),
        participant_transfer_pre_base64: BASE64.encode(&initial_participant_bytes),
        expected_source_base64: BASE64.encode(expected_source),
        expected_participant_token_base64: BASE64.encode(expected_participant),
        instructions: instructions_evidence,
        prestate,
    };
    authenticate_collateral_chain_join_v1(&intent)?;
    authenticate_collateral_economics_v1(&intent)?;
    Ok(CollateralReportV1 {
        phase: CollateralPhaseV1::Planned,
        intent_sha256: sha256_hex(&serde_json::to_vec(&intent)?),
        envelope_sha256: String::new(),
        intent,
        signed_packet_base64: None,
        signed_packet_sha256: None,
        expected_signature: None,
        finalized: None,
    })
}

fn collateral_observation_v1(slot: u64, block_time: i64) -> Observation {
    Observation {
        finality: Finality::Finalized,
        slot,
        unix_timestamp: block_time,
    }
}

fn participant_collateral_seed_v1(
    market: Pubkey,
    participant: Pubkey,
    release_set: [u8; 32],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIRECT_COLLATERAL_SEED_DOMAIN_V1);
    hasher.update(market.as_ref());
    hasher.update(participant.as_ref());
    hasher.update(release_set);
    hex(&hasher.finalize())[..32].to_owned()
}

fn token_instruction<const ACCOUNTS: usize, const DATA: usize>(
    spec: InstructionSpec<ACCOUNTS, DATA>,
) -> Instruction {
    Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: spec
            .accounts()
            .iter()
            .map(|account| AccountMeta {
                pubkey: Pubkey::new_from_array(*account.address()),
                is_signer: account.is_signer(),
                is_writable: account.is_writable(),
            })
            .collect(),
        data: spec.data().to_vec(),
    }
}

fn instruction_evidence(instruction: &Instruction) -> InstructionEvidenceV1 {
    InstructionEvidenceV1 {
        program_id: instruction.program_id.to_string(),
        accounts: instruction
            .accounts
            .iter()
            .map(|account| InstructionAccountV1 {
                address: account.pubkey.to_string(),
                signer: account.is_signer,
                writable: account.is_writable,
            })
            .collect(),
        data_base64: BASE64.encode(&instruction.data),
    }
}

fn decode_hex32(value: &str, label: &str) -> Result<[u8; 32]> {
    let bytes = decode_hex(value)?;
    bytes
        .try_into()
        .map_err(|_| Error::new(format!("{label} must be exactly 32 bytes")))
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(Error::new("hex text must have even width"));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|_| Error::new("hex was not UTF-8"))?;
            u8::from_str_radix(text, 16).map_err(|_| Error::new("hex contained a non-hex digit"))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn authenticate_collateral_instruction_sequence_v1(
    instructions: &[Instruction],
    creates_participant_account: bool,
    fee_payer: Pubkey,
    participant: Pubkey,
    participant_token: Pubkey,
    participant_seed: &str,
    collateral: &CollateralArgumentsV1,
    token_program: Pubkey,
    mint: Pubkey,
    custody_authority: Pubkey,
    decimals: u8,
    participant_rent: u64,
) -> Result<()> {
    let mut expected = Vec::new();
    if creates_participant_account {
        expected.push(create_account_with_seed(
            &fee_payer,
            &participant_token,
            &participant,
            participant_seed,
            participant_rent,
            u64::try_from(ACCOUNT_BYTES).map_err(|_| Error::new("token width overflow"))?,
            &token_program,
        ));
        expected.push(token_instruction(
            initialize_account3(
                token_program.to_bytes(),
                participant_token.to_bytes(),
                mint.to_bytes(),
                participant.to_bytes(),
            )
            .map_err(|error| Error::new(format!("InitializeAccount3: {error:?}")))?,
        ));
    }
    expected.push(token_instruction(
        transfer_checked(
            token_program.to_bytes(),
            collateral.source_account.to_bytes(),
            mint.to_bytes(),
            participant_token.to_bytes(),
            collateral.source_owner.to_bytes(),
            collateral.quantity_atoms,
            decimals,
        )
        .map_err(|error| Error::new(format!("TransferChecked: {error:?}")))?,
    ));
    expected.push(token_instruction(
        approve_checked(
            token_program.to_bytes(),
            participant_token.to_bytes(),
            mint.to_bytes(),
            custody_authority.to_bytes(),
            participant.to_bytes(),
            collateral.quantity_atoms,
            decimals,
        )
        .map_err(|error| Error::new(format!("ApproveChecked: {error:?}")))?,
    ));
    if instructions != expected {
        return Err(Error::new(
            "collateral instruction order, account roles, Mint, authority, decimals, or raw-atom quantity differed from the canonical sequence",
        ));
    }
    Ok(())
}

fn authenticate_collateral_arguments(
    report: &ReportV1,
    arguments: &CollateralArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &Value,
) -> Result<()> {
    let collateral = report
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("collateral report disappeared"))?;
    let admission_market = report
        .intent
        .prestate
        .get("core_market")
        .ok_or_else(|| Error::new("admission prestate omitted its Core Market"))?;
    let collateral_market = collateral
        .intent
        .prestate
        .get("market")
        .ok_or_else(|| Error::new("collateral prestate omitted its Market"))?;
    if collateral.intent.admission_intent_sha256 != report.intent_sha256
        || collateral.intent.participant != report.intent.position_owner
        || collateral.intent.fee_payer != report.intent.fee_payer
        || collateral.intent.source_owner != arguments.source_owner.to_string()
        || collateral.intent.source_account != arguments.source_account.to_string()
        || collateral.intent.quantity_atoms != arguments.quantity_atoms
        || collateral.intent.minimum_finalized_slot != report.intent.minimum_finalized_slot
        || collateral_market != admission_market
        || collateral.intent.market != admission_market.address
    {
        return Err(Error::new(
            "persisted collateral plan belongs to another admission, Core Market, participant, payer, source owner/account, quantity, or finalized floor",
        ));
    }
    authenticate_collateral_context_v1(&collateral.intent, plan, evidence)?;
    authenticate_collateral_intent_digest(collateral)
}

fn authenticate_collateral_context_v1(
    intent: &CollateralIntentV1,
    plan: &SuccessorPlan,
    evidence: &Value,
) -> Result<()> {
    let evidence_accounts = evidence_accounts(evidence)?;
    let realm_evidence = evidence_accounts
        .get("realm_record")
        .ok_or_else(|| Error::new("campaign evidence omitted realm_record"))?;
    let mint_evidence = evidence_accounts
        .get("collateral_mint")
        .ok_or_else(|| Error::new("campaign evidence omitted collateral_mint"))?;
    let programdata = decode_expected(
        &intent.custody_programdata_pre_base64,
        "Custody ProgramData prestate",
    )?;
    let mint_prestate = intent
        .prestate
        .get("mint")
        .ok_or_else(|| Error::new("durable collateral prestate omitted mint"))?;
    let expected = IndependentCollateralCoordinatesV1 {
        market: evidence_address(evidence, "founding_market")?.to_string(),
        realm_record: evidence_address(evidence, "realm_record")?.to_string(),
        mint: evidence_address(evidence, "collateral_mint")?.to_string(),
        registry_program: plan.registry.program_id.clone(),
        release_set: plan.release_set_id.clone(),
        core_program: plan.core.program_id.clone(),
        custody_program: plan.custody.program_id.clone(),
        custody_programdata: plan.custody.programdata_id.clone(),
        custody_artifact_release: plan.custody.artifact_release_id.clone(),
        realm_data_sha256: realm_evidence
            .get("data_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("Realm evidence omitted data_sha256"))?
            .into(),
        mint_data_sha256: mint_evidence
            .get("data_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("Mint evidence omitted data_sha256"))?
            .into(),
        custody_programdata_sha256: plan.custody.programdata_sha256.clone(),
    };
    authenticate_independent_collateral_coordinates_v1(
        intent,
        &expected,
        mint_prestate,
        &programdata,
    )
}

fn authenticate_independent_collateral_coordinates_v1(
    intent: &CollateralIntentV1,
    expected: &IndependentCollateralCoordinatesV1,
    mint_prestate: &AccountStateV1,
    custody_programdata: &[u8],
) -> Result<()> {
    let market_prestate = intent
        .prestate
        .get("market")
        .ok_or_else(|| Error::new("durable collateral prestate omitted market"))?;
    if intent.market != expected.market
        || intent.realm_record != expected.realm_record
        || intent.mint != expected.mint
        || intent.registry_program != expected.registry_program
        || intent.release_set != expected.release_set
        || intent.custody_program != expected.custody_program
        || intent.custody_programdata != expected.custody_programdata
        || intent.custody_artifact_release != expected.custody_artifact_release
        || market_prestate.owner != expected.core_program
        || intent.realm_data_sha256 != expected.realm_data_sha256
        || mint_prestate.data_sha256 != expected.mint_data_sha256
        || sha256_hex(custody_programdata) != expected.custody_programdata_sha256
    {
        return Err(Error::new(
            "durable collateral Market, Realm, Mint, release set, Core owner, or Custody deployment differed from the checked plan and campaign evidence",
        ));
    }
    Ok(())
}

fn authenticate_collateral_intent_digest(report: &CollateralReportV1) -> Result<()> {
    if sha256_hex(&serde_json::to_vec(&report.intent)?) != report.intent_sha256 {
        return Err(Error::new("durable collateral intent digest changed"));
    }
    authenticate_collateral_chain_join_v1(&report.intent)?;
    authenticate_collateral_token_byte_plan(&report.intent)?;
    authenticate_collateral_economics_v1(&report.intent)?;
    authenticate_collateral_message_plan(&report.intent)
}

fn authenticate_custody_activation_v1(snapshot: CustodyActivationSnapshotV1<'_>) -> Result<String> {
    let CustodyActivationSnapshotV1 {
        registry_program,
        release_set,
        activation_cache,
        activation_cache_account,
        custody_program,
        custody_program_account,
        custody_programdata,
        custody_programdata_account,
    } = snapshot;
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_slice()],
        &registry_program,
    )
    .0;
    if activation_cache != expected_cache
        || activation_cache_account.owner != registry_program
        || activation_cache_account.executable
        || activation_cache_account.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
    {
        return Err(Error::new(
            "Custody activation cache address, owner, privilege, or width refused",
        ));
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_cache_account.data)
        .map_err(|error| Error::new(format!("Custody activation cache: {error:?}")))?;
    if activated
        .execution_release_set_id()
        .map_err(|error| Error::new(format!("Custody activation release set: {error:?}")))?
        .to_bytes()
        != release_set
    {
        return Err(Error::new(
            "Custody activation cache selected another release set",
        ));
    }
    let selected = activated
        .role(ExecutionRoleV1::Custody)
        .map_err(|error| Error::new(format!("Custody activation role: {error:?}")))?;
    let observation = custody_deployment_observation_v1(
        custody_program,
        custody_program_account,
        custody_programdata,
        custody_programdata_account,
        selected.release(),
    )?;
    selected
        .authenticate_current_deployment(observation)
        .map_err(|error| Error::new(format!("current Custody deployment: {error:?}")))?;
    Ok(hex(&selected.artifact_release_id().to_bytes()))
}

fn custody_deployment_observation_v1(
    program: Pubkey,
    program_account: &RpcAccount,
    programdata: Pubkey,
    programdata_account: &RpcAccount,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1> {
    require_slot_pinned_release_v1(release)
        .map_err(|error| Error::new(format!("Custody slot-pin release: {error:?}")))?;
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.program().to_bytes() != program.to_bytes()
        || release.programdata() != programdata.to_bytes()
        || program_account.owner != bpf_loader_upgradeable::ID
        || !program_account.executable
        || programdata_account.owner != bpf_loader_upgradeable::ID
        || programdata_account.executable
    {
        return Err(Error::new(
            "Custody Program or ProgramData identity, owner, or executable bit refused",
        ));
    }
    let program_view = ProgramV3View::parse(&program_account.data)
        .map_err(|error| Error::new(format!("Custody Program: {error:?}")))?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.to_bytes() || programdata != expected_programdata {
        return Err(Error::new(
            "Custody Program did not carry its canonical ProgramData link",
        ));
    }
    let programdata_view = ProgramDataV3View::parse(&programdata_account.data)
        .map_err(|error| Error::new(format!("Custody ProgramData: {error:?}")))?;
    let observed_slot = programdata_view.deployment_slot();
    let observed_authority = programdata_view.upgrade_authority();
    let elf_digest = slot_pinned_release_elf_digest_v1(release, observed_authority, observed_slot)
        .map_err(|error| {
            Error::new(format!(
                "Custody release was superseded or substituted: {error:?}"
            ))
        })?;
    DeploymentObservationV1::new(
        program.to_bytes(),
        program_account.owner.to_bytes(),
        program_account.executable,
        programdata.to_bytes(),
        programdata_account.owner.to_bytes(),
        programdata_account.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        observed_slot,
        elf_digest,
        observed_authority,
    )
    .map_err(|error| Error::new(format!("Custody deployment observation: {error:?}")))
}

fn authenticate_collateral_chain_join_v1(intent: &CollateralIntentV1) -> Result<()> {
    let market = pubkey(&intent.market)?;
    let registry_program = pubkey(&intent.registry_program)?;
    let realm_record = pubkey(&intent.realm_record)?;
    let release_set = decode_hex32(&intent.release_set, "collateral release set")?;
    let activation_cache = pubkey(&intent.activation_cache)?;
    let custody_program = pubkey(&intent.custody_program)?;
    let custody_programdata = pubkey(&intent.custody_programdata)?;

    let market_bytes = decode_expected(&intent.market_pre_base64, "Market prestate")?;
    let realm_bytes = decode_expected(&intent.realm_pre_base64, "Realm prestate")?;
    let registry_program_bytes = decode_expected(
        &intent.registry_program_pre_base64,
        "Registry Program prestate",
    )?;
    let activation_bytes = decode_expected(
        &intent.activation_cache_pre_base64,
        "activation cache prestate",
    )?;
    let custody_program_bytes = decode_expected(
        &intent.custody_program_pre_base64,
        "Custody Program prestate",
    )?;
    let custody_programdata_bytes = decode_expected(
        &intent.custody_programdata_pre_base64,
        "Custody ProgramData prestate",
    )?;
    require_persisted_account_bytes_v1(
        &intent.prestate,
        "market",
        market,
        None,
        false,
        &market_bytes,
    )?;
    require_persisted_account_bytes_v1(
        &intent.prestate,
        "realm_record",
        realm_record,
        Some(registry_program),
        false,
        &realm_bytes,
    )?;
    require_persisted_account_bytes_v1(
        &intent.prestate,
        "registry_program",
        registry_program,
        Some(bpf_loader_upgradeable::ID),
        true,
        &registry_program_bytes,
    )?;
    let registry_program_view = ProgramV3View::parse(&registry_program_bytes)
        .map_err(|error| Error::new(format!("durable Registry Program: {error:?}")))?;
    let expected_registry_programdata =
        Pubkey::find_program_address(&[registry_program.as_ref()], &bpf_loader_upgradeable::ID).0;
    if registry_program_view.programdata() != expected_registry_programdata.to_bytes() {
        return Err(Error::new(
            "durable Registry Program did not carry its canonical Loader ProgramData link",
        ));
    }
    require_persisted_account_bytes_v1(
        &intent.prestate,
        "activation_cache",
        activation_cache,
        Some(registry_program),
        false,
        &activation_bytes,
    )?;
    require_persisted_account_bytes_v1(
        &intent.prestate,
        "custody_program",
        custody_program,
        Some(bpf_loader_upgradeable::ID),
        true,
        &custody_program_bytes,
    )?;
    require_persisted_account_bytes_v1(
        &intent.prestate,
        "custody_programdata",
        custody_programdata,
        Some(bpf_loader_upgradeable::ID),
        false,
        &custody_programdata_bytes,
    )?;

    let market_state = CoreState::decode(&market_bytes)
        .map_err(|error| Error::new(format!("durable collateral Market: {error:?}")))?;
    let realm_digest: [u8; 32] = Sha256::digest(&realm_bytes).into();
    let expected_realm = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            REALM_SCHEMA_RELEASE_ID_V1.as_slice(),
            realm_digest.as_slice(),
        ],
        &registry_program,
    )
    .0;
    if market_state.phase != CorePhase::Open
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
        || market_state.identity.registry_program.to_bytes() != registry_program.to_bytes()
        || market_state.identity.realm_id.to_bytes() != realm_digest
        || market_state.identity.selected_release_set.to_bytes() != release_set
        || realm_record != expected_realm
        || intent.realm_data_sha256 != sha256_hex(&realm_bytes)
    {
        return Err(Error::new(
            "durable Open Market, Registry, Realm, or release-set join changed",
        ));
    }
    let realm = RealmV1::decode(&realm_bytes)
        .map_err(|error| Error::new(format!("durable collateral Realm: {error:?}")))?;
    let expected_adapter_release: [u8; 32] = Sha256::digest(
        CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer().to_bytes(),
    )
    .into();
    if realm.token_program() != &TOKEN_2022_PROGRAM_ID
        || realm.collateral_mint() != &pubkey(&intent.mint)?.to_bytes()
        || realm.collateral_adapter_release_id() != &expected_adapter_release
        || intent.collateral_adapter_release != hex(&expected_adapter_release)
        || realm.mint_authority_policy() != MintAuthorityPolicy::RequireAbsent
        || realm.freeze_authority_policy() != FreezeAuthorityPolicy::RequireAbsent
    {
        return Err(Error::new(
            "durable Realm Mint or Token-2022 collateral release changed",
        ));
    }
    let activation_account = RpcAccount {
        lamports: intent.prestate["activation_cache"].lamports,
        owner: registry_program,
        executable: false,
        rent_epoch: intent.prestate["activation_cache"].rent_epoch,
        data: activation_bytes,
    };
    let custody_program_account = RpcAccount {
        lamports: intent.prestate["custody_program"].lamports,
        owner: bpf_loader_upgradeable::ID,
        executable: true,
        rent_epoch: intent.prestate["custody_program"].rent_epoch,
        data: custody_program_bytes,
    };
    let custody_programdata_account = RpcAccount {
        lamports: intent.prestate["custody_programdata"].lamports,
        owner: bpf_loader_upgradeable::ID,
        executable: false,
        rent_epoch: intent.prestate["custody_programdata"].rent_epoch,
        data: custody_programdata_bytes,
    };
    let artifact = authenticate_custody_activation_v1(CustodyActivationSnapshotV1 {
        registry_program,
        release_set,
        activation_cache,
        activation_cache_account: &activation_account,
        custody_program,
        custody_program_account: &custody_program_account,
        custody_programdata,
        custody_programdata_account: &custody_programdata_account,
    })?;
    if artifact != intent.custody_artifact_release {
        return Err(Error::new(
            "durable activated Custody artifact release changed",
        ));
    }
    Ok(())
}

fn require_persisted_account_bytes_v1(
    prestate: &BTreeMap<String, AccountStateV1>,
    label: &str,
    address: Pubkey,
    owner: Option<Pubkey>,
    executable: bool,
    bytes: &[u8],
) -> Result<()> {
    let state = prestate
        .get(label)
        .ok_or_else(|| Error::new(format!("durable collateral prestate omitted {label}")))?;
    let actual_owner = pubkey(&state.owner)?;
    let exact = RpcAccount {
        lamports: state.lamports,
        owner: actual_owner,
        executable: state.executable,
        rent_epoch: state.rent_epoch,
        data: bytes.to_vec(),
    };
    if parse_state_address(state)? != address
        || owner.is_some_and(|owner| state.owner != owner.to_string())
        || state.executable != executable
        || account_state(address, Some(&exact)) != *state
    {
        return Err(Error::new(format!(
            "durable collateral {label} address, owner, privilege, width, or bytes changed"
        )));
    }
    Ok(())
}

fn authenticate_collateral_economics_v1(intent: &CollateralIntentV1) -> Result<()> {
    let payer = intent
        .prestate
        .get("fee_payer")
        .ok_or_else(|| Error::new("durable collateral prestate omitted fee_payer"))?;
    if parse_state_address(payer)?.to_string() != intent.fee_payer
        || payer.owner != system_program::ID.to_string()
        || payer.executable
        || payer.data_len != 0
        || payer.data_sha256 != sha256_hex(&[])
        || (!intent.creates_participant_account && intent.participant_account_rent_lamports != 0)
        || intent
            .participant_account_rent_lamports
            .checked_add(intent.transaction_fee_lamports)
            != Some(intent.total_fee_payer_debit_lamports)
        || payer.lamports < intent.total_fee_payer_debit_lamports
    {
        return Err(Error::new(
            "durable collateral payer, create-mode rent, fee total, or balance changed",
        ));
    }
    Ok(())
}

fn authenticate_collateral_token_byte_plan(intent: &CollateralIntentV1) -> Result<()> {
    let token_program_key = pubkey(&intent.token_program)?;
    let market = pubkey(&intent.market)?;
    let release_set = decode_hex32(&intent.release_set, "collateral release set")?;
    let participant = pubkey(&intent.participant)?;
    let custody_program = pubkey(&intent.custody_program)?;
    let custody_authority = pubkey(&intent.custody_authority)?;
    let expected_adapter_release: [u8; 32] = Sha256::digest(
        CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer().to_bytes(),
    )
    .into();
    let expected_custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release_set).as_slices(),
        &custody_program,
    )
    .0;
    let participant_seed = participant_collateral_seed_v1(market, participant, release_set);
    let participant_account =
        Pubkey::create_with_seed(&participant, &participant_seed, &token_program_key)
            .map_err(|error| Error::new(format!("participant token derivation: {error}")))?;
    if token_program_key.to_bytes() != TOKEN_2022_PROGRAM_ID
        || intent.collateral_adapter_release != hex(&expected_adapter_release)
        || custody_authority != expected_custody_authority
        || intent.participant_token_seed != participant_seed
        || pubkey(&intent.participant_token_account)? != participant_account
    {
        return Err(Error::new(
            "durable Token-2022 release, Custody authority, or participant account derivation changed",
        ));
    }
    let token_program = &intent.token_program;
    let mint = pubkey(&intent.mint)?;
    let source_owner = pubkey(&intent.source_owner)?;
    let mint_before = intent
        .prestate
        .get("mint")
        .ok_or_else(|| Error::new("durable collateral prestate omitted mint"))?;
    let source_before = intent
        .prestate
        .get("source_account")
        .ok_or_else(|| Error::new("durable collateral prestate omitted source_account"))?;
    let participant_before = intent
        .prestate
        .get("participant_token_account")
        .ok_or_else(|| {
            Error::new("durable collateral prestate omitted participant_token_account")
        })?;
    if parse_state_address(mint_before)? != mint
        || parse_state_address(source_before)? != pubkey(&intent.source_account)?
        || parse_state_address(participant_before)? != participant_account
    {
        return Err(Error::new(
            "durable source or participant prestate address changed",
        ));
    }
    let mint_pre = decode_expected(&intent.mint_pre_base64, "collateral Mint prestate")?;
    require_account_bytes(
        mint_before,
        &mint_pre,
        token_program,
        mint_before.lamports,
        "collateral Mint prestate",
    )?;
    let parsed_mint = Mint::parse(&mint_pre)
        .map_err(|error| Error::new(format!("collateral Mint prestate: {error:?}")))?;
    if !parsed_mint.is_initialized
        || !parsed_mint.mint_authority.is_none()
        || !parsed_mint.freeze_authority.is_none()
        || parsed_mint.decimals != intent.mint_decimals
    {
        return Err(Error::new(
            "durable collateral Mint initialization, authority, or decimals changed",
        ));
    }
    let source_pre = decode_expected(&intent.source_pre_base64, "source token prestate")?;
    let source = TokenAccount::parse(&source_pre)
        .map_err(|error| Error::new(format!("source token prestate: {error:?}")))?;
    if source.mint != mint.to_bytes()
        || source.owner != source_owner.to_bytes()
        || source.state != TokenAccountState::Initialized
        || !source.native_reserve.is_none()
        || source.amount < intent.quantity_atoms
    {
        return Err(Error::new(
            "durable source token prestate changed Mint, owner, state, native profile, or raw atoms",
        ));
    }
    require_account_bytes(
        source_before,
        &source_pre,
        token_program,
        source_before.lamports,
        "source token prestate",
    )?;
    let expected_source_amount = source
        .amount
        .checked_sub(intent.quantity_atoms)
        .ok_or_else(|| Error::new("source raw-atom subtraction underflow"))?;
    let projected_source =
        TokenAccount::project_amount_poststate(&source_pre, expected_source_amount)
            .map_err(|error| Error::new(format!("source token projection: {error:?}")))?;
    if BASE64.encode(projected_source) != intent.expected_source_base64 {
        return Err(Error::new(
            "durable source token poststate was not the exact raw-atom projection",
        ));
    }

    let participant_transfer_pre = decode_expected(
        &intent.participant_transfer_pre_base64,
        "participant token transfer prestate",
    )?;
    let participant_token = TokenAccount::parse(&participant_transfer_pre)
        .map_err(|error| Error::new(format!("participant token prestate: {error:?}")))?;
    if participant_token.mint != mint.to_bytes()
        || participant_token.owner != participant.to_bytes()
        || participant_token.amount != 0
        || participant_token.state != TokenAccountState::Initialized
        || !participant_token.delegate.is_none()
        || participant_token.delegated_amount != 0
        || !participant_token.native_reserve.is_none()
        || !participant_token.close_authority.is_none()
    {
        return Err(Error::new(
            "durable participant transfer prestate was not the exact empty base-token profile",
        ));
    }
    match (
        intent.creates_participant_account,
        intent.participant_pre_base64.as_deref(),
    ) {
        (true, None) => {
            if participant_before.owner != system_program::ID.to_string()
                || participant_before.lamports != 0
                || participant_before.executable
                || participant_before.data_len != 0
                || participant_before.data_sha256 != sha256_hex(&[])
                || participant_transfer_pre
                    != TokenAccount::initialized_base_bytes(mint.to_bytes(), participant.to_bytes())
                        .map_err(|error| {
                            Error::new(format!("participant initialization: {error:?}"))
                        })?
            {
                return Err(Error::new(
                    "durable participant creation prestate or InitializeAccount3 bytes changed",
                ));
            }
        }
        (false, Some(before)) => {
            let before = BASE64
                .decode(before)
                .map_err(|error| Error::new(format!("participant token prestate: {error}")))?;
            if before != participant_transfer_pre {
                return Err(Error::new(
                    "existing participant prestate differed from its transfer prestate",
                ));
            }
            require_account_bytes(
                participant_before,
                &before,
                token_program,
                participant_before.lamports,
                "participant token prestate",
            )?;
        }
        _ => {
            return Err(Error::new(
                "participant prestate presence differed from its durable create decision",
            ));
        }
    }
    let projected_participant = TokenAccount::project_delegated_source_poststate(
        &participant_transfer_pre,
        intent.quantity_atoms,
        COption::Some(custody_authority.to_bytes()),
        intent.quantity_atoms,
    )
    .map_err(|error| Error::new(format!("participant token projection: {error:?}")))?;
    if BASE64.encode(projected_participant) != intent.expected_participant_token_base64 {
        return Err(Error::new(
            "durable participant poststate did not establish the exact Custody allowance",
        ));
    }
    Ok(())
}

fn canonical_collateral_instructions(intent: &CollateralIntentV1) -> Result<Vec<Instruction>> {
    let fee_payer = parse_state_address(
        intent
            .prestate
            .get("fee_payer")
            .ok_or_else(|| Error::new("durable collateral prestate omitted fee_payer"))?,
    )?;
    let participant = pubkey(&intent.participant)?;
    let participant_token = pubkey(&intent.participant_token_account)?;
    let token_program = pubkey(&intent.token_program)?;
    let mint = pubkey(&intent.mint)?;
    let custody_authority = pubkey(&intent.custody_authority)?;
    let collateral = CollateralArgumentsV1 {
        source_owner: pubkey(&intent.source_owner)?,
        source_owner_keypair: PathBuf::new(),
        source_account: pubkey(&intent.source_account)?,
        quantity_atoms: intent.quantity_atoms,
    };
    let mut instructions = Vec::new();
    if intent.creates_participant_account {
        instructions.push(create_account_with_seed(
            &fee_payer,
            &participant_token,
            &participant,
            &intent.participant_token_seed,
            intent.participant_account_rent_lamports,
            u64::try_from(ACCOUNT_BYTES).map_err(|_| Error::new("token width overflow"))?,
            &token_program,
        ));
        instructions.push(token_instruction(
            initialize_account3(
                token_program.to_bytes(),
                participant_token.to_bytes(),
                mint.to_bytes(),
                participant.to_bytes(),
            )
            .map_err(|error| Error::new(format!("InitializeAccount3: {error:?}")))?,
        ));
    }
    instructions.push(token_instruction(
        transfer_checked(
            token_program.to_bytes(),
            collateral.source_account.to_bytes(),
            mint.to_bytes(),
            participant_token.to_bytes(),
            collateral.source_owner.to_bytes(),
            collateral.quantity_atoms,
            intent.mint_decimals,
        )
        .map_err(|error| Error::new(format!("TransferChecked: {error:?}")))?,
    ));
    instructions.push(token_instruction(
        approve_checked(
            token_program.to_bytes(),
            participant_token.to_bytes(),
            mint.to_bytes(),
            custody_authority.to_bytes(),
            participant.to_bytes(),
            collateral.quantity_atoms,
            intent.mint_decimals,
        )
        .map_err(|error| Error::new(format!("ApproveChecked: {error:?}")))?,
    ));
    authenticate_collateral_instruction_sequence_v1(
        &instructions,
        intent.creates_participant_account,
        fee_payer,
        participant,
        participant_token,
        &intent.participant_token_seed,
        &collateral,
        token_program,
        mint,
        custody_authority,
        intent.mint_decimals,
        intent.participant_account_rent_lamports,
    )?;
    Ok(instructions)
}

fn authenticate_collateral_message_plan(intent: &CollateralIntentV1) -> Result<()> {
    let instructions = canonical_collateral_instructions(intent)?;
    let evidence = instructions
        .iter()
        .map(instruction_evidence)
        .collect::<Vec<_>>();
    if evidence != intent.instructions {
        return Err(Error::new(
            "durable collateral instruction evidence differed from the canonical sequence",
        ));
    }
    let fee_payer = parse_state_address(
        intent
            .prestate
            .get("fee_payer")
            .ok_or_else(|| Error::new("durable collateral prestate omitted fee_payer"))?,
    )?;
    let recent_blockhash = intent
        .recent_blockhash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("durable collateral blockhash: {error}")))?;
    let compiled = compile_v0_message_with_optional_tables(
        fee_payer,
        &instructions,
        recent_blockhash,
        Observation {
            finality: Finality::Finalized,
            slot: intent.observation_slot,
            unix_timestamp: intent.observation_unix_timestamp,
        },
        &[],
    )
    .map_err(|error| Error::new(format!("recompile collateral message: {error:?}")))?;
    let message = compiled.message.serialize();
    if compiled.wire_bytes != intent.wire_bytes
        || BASE64.encode(&message) != intent.message_base64
        || sha256_hex(&message) != intent.message_sha256
        || compiled
            .message
            .address_table_lookups()
            .is_some_and(|lookups| !lookups.is_empty())
    {
        return Err(Error::new(
            "durable collateral message was not the exact canonical no-ALT packet",
        ));
    }
    Ok(())
}

fn resume_collateral(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    collateral_arguments: &CollateralArgumentsV1,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
) -> Result<()> {
    let phase = report
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("collateral report disappeared"))?
        .phase;
    if phase == CollateralPhaseV1::Finalized {
        return verify_persisted_collateral(rpc, report);
    }
    let complete = probe_collateral_poststate(rpc, report, 0)?;
    if complete {
        let signature = report
            .collateral
            .as_ref()
            .and_then(|value| value.expected_signature.clone())
            .ok_or_else(|| {
                Error::new(
                    "REFUSED: exact delegated collateral poststate exists without this journal's signature; never infer ownership or replay the transfer",
                )
            })?;
        finalize_collateral_signature(rpc, report, &signature)?;
        journal.persist(report)?;
        return Ok(());
    }
    match phase {
        CollateralPhaseV1::Planned => {
            sign_and_submit_collateral(rpc, arguments, collateral_arguments, report, journal)
        }
        CollateralPhaseV1::SignedNotSubmitted | CollateralPhaseV1::Submitted => {
            let signature = report
                .collateral
                .as_ref()
                .and_then(|value| value.expected_signature.clone())
                .ok_or_else(|| Error::new("signed collateral journal omitted its signature"))?;
            match finalized_transaction(rpc, &signature)? {
                Some(_) => {
                    finalize_collateral_signature(rpc, report, &signature)?;
                    journal.persist(report)
                }
                None => Err(Error::new(format!(
                    "REFUSED: collateral transaction {signature} is not finalized and its exact poststate is absent. Phase {phase:?} is ambiguous, so the executor will neither sign again nor resend the durable packet"
                ))),
            }
        }
        CollateralPhaseV1::Finalized => Ok(()),
    }
}

fn sign_and_submit_collateral(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    collateral_arguments: &CollateralArgumentsV1,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
) -> Result<()> {
    authenticate_collateral_intent_digest(
        report
            .collateral
            .as_ref()
            .ok_or_else(|| Error::new("collateral report disappeared"))?,
    )?;
    require_collateral_prestate_unchanged(rpc, report)?;
    let intent = &report
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("collateral report disappeared"))?
        .intent;
    if intent.fee_payer != arguments.fee_payer.to_string()
        || intent.fee_payer != report.intent.fee_payer
    {
        return Err(Error::new(
            "collateral fee payer differed from the admission and current CLI payer",
        ));
    }
    authenticate_fresh_collateral_economics_v1(rpc, intent)?;
    let height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if height > intent.last_valid_block_height {
        return Err(Error::new(
            "collateral blockhash expired before key load; archive this unsigned collateral plan and construct a fresh finalized plan",
        ));
    }
    authenticate_genesis_again(rpc, &arguments.origin)?;
    enforce_shared_key_path(
        arguments.position_owner,
        &arguments.position_owner_keypair,
        collateral_arguments.source_owner,
        &collateral_arguments.source_owner_keypair,
    )?;
    enforce_shared_key_path(
        arguments.position_owner,
        &arguments.position_owner_keypair,
        arguments.fee_payer,
        &arguments.fee_payer_keypair,
    )?;
    enforce_shared_key_path(
        collateral_arguments.source_owner,
        &collateral_arguments.source_owner_keypair,
        arguments.fee_payer,
        &arguments.fee_payer_keypair,
    )?;

    // First key access for this second transaction occurs only after its full
    // durable journal exists.
    let participant = read_expected_keypair(
        &arguments.position_owner_keypair,
        arguments.position_owner,
        "participant",
    )?;
    let source_owner = if collateral_arguments.source_owner == arguments.position_owner {
        None
    } else {
        Some(read_expected_keypair(
            &collateral_arguments.source_owner_keypair,
            collateral_arguments.source_owner,
            "collateral source owner",
        )?)
    };
    let fee_payer = if arguments.fee_payer == arguments.position_owner
        || arguments.fee_payer == collateral_arguments.source_owner
    {
        None
    } else {
        Some(read_expected_keypair(
            &arguments.fee_payer_keypair,
            arguments.fee_payer,
            "collateral fee payer",
        )?)
    };
    let message_bytes = BASE64
        .decode(&intent.message_base64)
        .map_err(|error| Error::new(format!("persisted collateral message base64: {error}")))?;
    if sha256_hex(&message_bytes) != intent.message_sha256 {
        return Err(Error::new("persisted collateral message digest changed"));
    }
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("persisted collateral message: {error}")))?;
    if message
        .address_table_lookups()
        .is_some_and(|lookups| !lookups.is_empty())
    {
        return Err(Error::new(
            "collateral transaction unexpectedly depended on a lookup table",
        ));
    }
    let mut signers: Vec<&dyn Signer> = vec![&participant];
    if let Some(source_owner) = source_owner.as_ref() {
        signers.push(source_owner);
    }
    if let Some(fee_payer) = fee_payer.as_ref() {
        signers.push(fee_payer);
    }
    let transaction = VersionedTransaction::try_new(message, &signers)
        .map_err(|error| Error::new(format!("sign collateral transaction: {error}")))?;
    let signature = transaction
        .signatures
        .first()
        .copied()
        .ok_or_else(|| Error::new("signed collateral transaction omitted payer signature"))?;
    let wire = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize signed collateral: {error}")))?;
    if wire.len() != intent.wire_bytes {
        return Err(Error::new(format!(
            "signed collateral packet has {} bytes; journal committed {}",
            wire.len(),
            intent.wire_bytes
        )));
    }
    {
        let collateral = report
            .collateral
            .as_mut()
            .ok_or_else(|| Error::new("collateral report disappeared"))?;
        collateral.signed_packet_base64 = Some(BASE64.encode(&wire));
        collateral.signed_packet_sha256 = Some(sha256_hex(&wire));
        collateral.expected_signature = Some(signature.to_string());
        collateral.phase = CollateralPhaseV1::Submitted;
    }
    journal.persist(report)?;

    authenticate_genesis_again(rpc, &arguments.origin)?;
    require_collateral_prestate_unchanged(rpc, report)?;
    let returned = rpc
        .call_once(
            "sendTransaction",
            &json!([BASE64.encode(&wire), {
                "encoding":"base64",
                "skipPreflight":false,
                "preflightCommitment":"finalized",
                "maxRetries":0
            }]),
        )?
        .as_str()
        .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("collateral signature: {error}")))?;
    if returned != signature {
        return Err(Error::new(
            "RPC returned a signature different from the durable collateral packet",
        ));
    }
    wait_finalized(rpc, &signature.to_string())?;
    finalize_collateral_signature(rpc, report, &signature.to_string())?;
    journal.persist(report)
}

fn authenticate_fresh_collateral_economics_v1(
    rpc: &mut Rpc,
    intent: &CollateralIntentV1,
) -> Result<()> {
    let fresh_rent = rpc.minimum_balance(ACCOUNT_BYTES)?;
    let fresh_fee = fee_for_message(rpc, &intent.message_base64)?;
    authenticate_collateral_economics_against_fresh_v1(intent, fresh_rent, fresh_fee)
}

fn authenticate_collateral_economics_against_fresh_v1(
    intent: &CollateralIntentV1,
    fresh_account_rent: u64,
    fresh_fee: u64,
) -> Result<()> {
    authenticate_collateral_economics_v1(intent)?;
    let expected_rent = if intent.creates_participant_account {
        fresh_account_rent
    } else {
        0
    };
    let fresh_total = expected_rent
        .checked_add(fresh_fee)
        .ok_or_else(|| Error::new("fresh collateral rent+fee overflow"))?;
    if intent.participant_account_rent_lamports != expected_rent
        || intent.transaction_fee_lamports != fresh_fee
        || intent.total_fee_payer_debit_lamports != fresh_total
    {
        return Err(Error::new(
            "fresh collateral account rent, exact message fee, or total payer debit changed before key load",
        ));
    }
    Ok(())
}

fn enforce_shared_key_path(
    first_key: Pubkey,
    first_path: &Path,
    second_key: Pubkey,
    second_path: &Path,
) -> Result<()> {
    if first_key == second_key && first_path != second_path {
        return Err(Error::new(
            "one signer public key was supplied through different keypair paths",
        ));
    }
    Ok(())
}

fn read_expected_keypair(path: &Path, expected: Pubkey, label: &str) -> Result<Keypair> {
    let keypair = Keypair::new_from_array(campaign::read_keypair_file(path, label)?);
    if keypair.pubkey() != expected {
        return Err(Error::new(format!(
            "{label} keypair does not expand to its explicit public key"
        )));
    }
    Ok(keypair)
}

fn probe_collateral_poststate(rpc: &mut Rpc, report: &ReportV1, floor: u64) -> Result<bool> {
    let intent = &report
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("collateral report disappeared"))?
        .intent;
    let source = pubkey(&intent.source_account)?;
    let participant = pubkey(&intent.participant_token_account)?;
    let (_, values) = rpc.finalized_accounts(&[source, participant], floor)?;
    let expected_source = decode_expected(&intent.expected_source_base64, "collateral source")?;
    let expected_participant = decode_expected(
        &intent.expected_participant_token_base64,
        "participant token account",
    )?;
    let token_program = pubkey(&intent.token_program)?;
    match (&values[0], &values[1]) {
        (Some(source), Some(participant)) => {
            let exact = source.owner == token_program
                && source.data == expected_source
                && participant.owner == token_program
                && participant.data == expected_participant;
            if exact {
                Ok(true)
            } else {
                let before_source = &intent.prestate["source_account"];
                let before_participant = &intent.prestate["participant_token_account"];
                if account_state(pubkey(&intent.source_account)?, Some(source)) == *before_source
                    && account_state(
                        pubkey(&intent.participant_token_account)?,
                        Some(participant),
                    ) == *before_participant
                {
                    Ok(false)
                } else {
                    Err(Error::new(
                        "REFUSED: source or participant token account is neither the durable prestate nor exact delegated poststate",
                    ))
                }
            }
        }
        (Some(source), None) if intent.creates_participant_account => {
            if account_state(pubkey(&intent.source_account)?, Some(source))
                == intent.prestate["source_account"]
            {
                Ok(false)
            } else {
                Err(Error::new(
                    "REFUSED: collateral source changed while the derived participant account remained absent",
                ))
            }
        }
        _ => Err(Error::new(
            "REFUSED: collateral source or participant account has an impossible presence shape",
        )),
    }
}

fn require_collateral_prestate_unchanged(rpc: &mut Rpc, report: &ReportV1) -> Result<()> {
    let intent = &report
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("collateral report disappeared"))?
        .intent;
    let labels = intent.prestate.keys().cloned().collect::<Vec<_>>();
    let addresses = labels
        .iter()
        .map(|label| parse_state_address(&intent.prestate[label]))
        .collect::<Result<Vec<_>>>()?;
    let (_, values) = rpc.finalized_accounts(&addresses, intent.observation_slot)?;
    for ((label, address), value) in labels.iter().zip(addresses).zip(values) {
        if account_state(address, value.as_ref()) != intent.prestate[label] {
            return Err(Error::new(format!(
                "REFUSED: collateral {label} changed after the durable finalized plan"
            )));
        }
    }
    Ok(())
}

fn authenticate_collateral_packet(report: &CollateralReportV1) -> Result<VersionedTransaction> {
    let packet = report
        .signed_packet_base64
        .as_ref()
        .ok_or_else(|| Error::new("signed collateral phase omitted packet"))?;
    let wire = BASE64
        .decode(packet)
        .map_err(|error| Error::new(format!("collateral packet base64: {error}")))?;
    if wire.len() != report.intent.wire_bytes {
        return Err(Error::new("durable collateral packet width changed"));
    }
    let transaction: VersionedTransaction = bincode::deserialize(&wire)
        .map_err(|error| Error::new(format!("durable collateral packet: {error}")))?;
    if report.signed_packet_sha256.as_deref() != Some(&sha256_hex(&wire)) {
        return Err(Error::new("durable collateral packet digest changed"));
    }
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("durable collateral packet signatures: {error}")))?;
    let message = transaction.message.serialize();
    if BASE64.encode(&message) != report.intent.message_base64
        || sha256_hex(&message) != report.intent.message_sha256
        || transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
            != report.expected_signature.as_deref()
    {
        return Err(Error::new(
            "durable collateral packet message, digest, or payer signature changed",
        ));
    }
    Ok(transaction)
}

fn authenticate_admission_packet(report: &ReportV1) -> Result<VersionedTransaction> {
    let packet = report
        .signed_packet_base64
        .as_ref()
        .ok_or_else(|| Error::new("signed admission phase omitted packet"))?;
    let wire = BASE64
        .decode(packet)
        .map_err(|error| Error::new(format!("admission packet base64: {error}")))?;
    if wire.len() != report.intent.wire_bytes {
        return Err(Error::new("durable admission packet width changed"));
    }
    let transaction: VersionedTransaction = bincode::deserialize(&wire)
        .map_err(|error| Error::new(format!("durable admission packet: {error}")))?;
    if report.signed_packet_sha256.as_deref() != Some(&sha256_hex(&wire)) {
        return Err(Error::new("durable admission packet digest changed"));
    }
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("durable admission packet signatures: {error}")))?;
    let message = transaction.message.serialize();
    if BASE64.encode(&message) != report.intent.message_base64
        || sha256_hex(&message) != report.intent.message_sha256
        || transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
            != report.expected_signature.as_deref()
    {
        return Err(Error::new(
            "durable admission packet message, digest, or payer signature changed",
        ));
    }
    Ok(transaction)
}

fn finalize_collateral_signature(
    rpc: &mut Rpc,
    report: &mut ReportV1,
    signature: &str,
) -> Result<()> {
    let collateral = report
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("collateral report disappeared"))?;
    let history = authenticate_collateral_finalized_history(rpc, collateral, signature)?;
    let poststate = verify_collateral_poststate(rpc, &collateral.intent, history.slot)?;
    let finalized = CollateralFinalizedEvidenceV1 {
        signature: history.signature,
        slot: history.slot,
        fee_lamports: history.fee_lamports,
        compute_units_consumed: history.compute_units_consumed,
        return_data: history.return_data,
        poststate,
    };
    let collateral = report
        .collateral
        .as_mut()
        .ok_or_else(|| Error::new("collateral report disappeared"))?;
    collateral.finalized = Some(finalized);
    collateral.phase = CollateralPhaseV1::Finalized;
    Ok(())
}

fn authenticate_collateral_finalized_history(
    rpc: &mut Rpc,
    collateral: &CollateralReportV1,
    signature: &str,
) -> Result<CollateralHistoryV1> {
    authenticate_collateral_intent_digest(collateral)?;
    let packet = authenticate_collateral_packet(collateral)?;
    if collateral.expected_signature.as_deref() != Some(signature) {
        return Err(Error::new(
            "finalized collateral signature differs from the durable packet",
        ));
    }
    let transaction = finalized_transaction(rpc, signature)?.ok_or_else(|| {
        Error::new(format!(
            "collateral signature {signature} has not reached finalized history"
        ))
    })?;
    if transaction
        .pointer("/transaction/signatures/0")
        .and_then(Value::as_str)
        != Some(signature)
    {
        return Err(Error::new(
            "finalized transaction history omitted the exact collateral signature",
        ));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized collateral transaction omitted slot"))?;
    let (wire_slot, history_wire) = finalized_transaction_wire(rpc, signature)?;
    let expected_wire = collateral
        .signed_packet_base64
        .as_deref()
        .ok_or_else(|| Error::new("finalized collateral journal omitted signed packet"))?;
    require_exact_history_wire_v1(expected_wire, &history_wire)?;
    if wire_slot != slot {
        return Err(Error::new(
            "finalized collateral JSON and exact-wire observations disagreed on slot",
        ));
    }
    if slot < collateral.intent.observation_slot || slot < collateral.intent.minimum_finalized_slot
    {
        return Err(Error::new(
            "finalized collateral transaction preceded its durable finalized observation",
        ));
    }
    let meta = transaction
        .get("meta")
        .ok_or_else(|| Error::new("finalized collateral transaction omitted meta"))?;
    if meta.get("err").is_none_or(|value| !value.is_null()) {
        return Err(Error::new(format!(
            "finalized collateral transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized collateral transaction omitted fee"))?;
    if fee != collateral.intent.transaction_fee_lamports {
        return Err(Error::new(format!(
            "finalized collateral fee {fee} differs from planned {}",
            collateral.intent.transaction_fee_lamports
        )));
    }
    authenticate_collateral_return_data(
        meta.get("returnData"),
        collateral.intent.expected_return_data.as_deref(),
    )?;
    authenticate_collateral_balance_vector(meta, &packet.message, &collateral.intent)?;
    Ok(CollateralHistoryV1 {
        signature: signature.into(),
        slot,
        fee_lamports: fee,
        compute_units_consumed: meta.get("computeUnitsConsumed").and_then(Value::as_u64),
        return_data: None,
    })
}

fn finalized_transaction_wire(rpc: &mut Rpc, signature: &str) -> Result<(u64, Vec<u8>)> {
    let transaction = rpc.call(
        "getTransaction",
        &json!([signature, {
            "encoding":"base64",
            "commitment":"finalized",
            "maxSupportedTransactionVersion":0
        }]),
    )?;
    if transaction.is_null() {
        return Err(Error::new(
            "finalized signature omitted exact transaction bytes",
        ));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("exact finalized transaction omitted slot"))?;
    let wire = decode_exact_base64_tuple_v1(
        transaction
            .get("transaction")
            .ok_or_else(|| Error::new("exact finalized transaction omitted packet tuple"))?,
        "exact finalized transaction packet",
    )?;
    Ok((slot, wire))
}

fn decode_exact_base64_tuple_v1(value: &Value, label: &str) -> Result<Vec<u8>> {
    let tuple = value
        .as_array()
        .ok_or_else(|| Error::new(format!("{label} was not an encoding tuple")))?;
    if tuple.len() != 2 || tuple[1].as_str() != Some("base64") {
        return Err(Error::new(format!(
            "{label} did not select the exact base64 encoding"
        )));
    }
    let encoded = tuple[0]
        .as_str()
        .ok_or_else(|| Error::new(format!("{label} body was not a string")))?;
    BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn require_exact_history_wire_v1(expected_base64: &str, observed: &[u8]) -> Result<()> {
    let expected = BASE64
        .decode(expected_base64)
        .map_err(|error| Error::new(format!("durable signed packet base64: {error}")))?;
    if expected != observed {
        return Err(Error::new(
            "finalized history was not the exact durable signed packet",
        ));
    }
    Ok(())
}

fn authenticate_collateral_return_data(
    observed: Option<&Value>,
    expected: Option<&str>,
) -> Result<()> {
    // The RPC omits `returnData` entirely when a transaction set none; an
    // absent field and an explicit null are the same finalized fact.
    if observed.is_some_and(|value| !value.is_null()) || expected.is_some() {
        return Err(Error::new(
            "System/Token-2022 collateral transaction must have no returnData",
        ));
    }
    Ok(())
}

fn authenticate_admission_balance_vector(
    meta: &Value,
    message: &VersionedMessage,
    intent: &IntentV1,
) -> Result<BTreeMap<Pubkey, u64>> {
    let pre = meta
        .get("preBalances")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("finalized admission meta omitted preBalances"))?;
    let post = meta
        .get("postBalances")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("finalized admission meta omitted postBalances"))?;
    // A routed admission's balance vectors cover the static keys and then the
    // loaded writable and readonly addresses, in the runtime's own order. The
    // loaded keys come from the finalized meta itself; every loaded account
    // must be balance-immutable here (the admission loads only readonly
    // program, record, and sysvar addresses through its frozen table), and
    // the whole-vector fee conservation below runs over all of them.
    let mut keys: Vec<Pubkey> = message.static_account_keys().to_vec();
    let static_len = keys.len();
    if let Some(loaded) = meta.get("loadedAddresses") {
        for section in ["writable", "readonly"] {
            for value in loaded
                .get(section)
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[])
            {
                let key = value
                    .as_str()
                    .ok_or_else(|| Error::new("loaded address was not a string"))?
                    .parse::<Pubkey>()
                    .map_err(|error| Error::new(format!("loaded address: {error}")))?;
                keys.push(key);
            }
        }
    }
    if pre.len() != keys.len() || post.len() != keys.len() {
        return Err(Error::new(
            "admission balance vector did not match exact static-plus-loaded message keys",
        ));
    }
    let mut durable_lamports = BTreeMap::new();
    for state in intent.prestate.values() {
        let key = parse_state_address(state)?;
        if durable_lamports
            .insert(key, state.lamports)
            .is_some_and(|prior| prior != state.lamports)
        {
            return Err(Error::new(
                "durable admission aliases disagreed on pre-balance",
            ));
        }
    }
    let owner = pubkey(&intent.position_owner)?;
    let payer = pubkey(&intent.fee_payer)?;
    let position = pubkey(&intent.position)?;
    let admission = pubkey(&intent.admission)?;
    let mut pre_sum = 0_u128;
    let mut post_sum = 0_u128;
    let mut finalized_balances = BTreeMap::new();
    for (index, ((key, before), after)) in keys.iter().zip(pre).zip(post).enumerate() {
        let before = before
            .as_u64()
            .ok_or_else(|| Error::new("admission preBalances contained a non-u64"))?;
        let after = after
            .as_u64()
            .ok_or_else(|| Error::new("admission postBalances contained a non-u64"))?;
        if index >= static_len {
            // Loaded addresses: not part of the durable prestate set, and the
            // admission may not move a lamport through any of them.
            if before != after {
                return Err(Error::new(format!(
                    "loaded address {key} changed balance inside the admission"
                )));
            }
            pre_sum += u128::from(before);
            post_sum += u128::from(after);
            continue;
        }
        if durable_lamports.get(key).copied() != Some(before) {
            return Err(Error::new(format!(
                "admission pre-balance for {key} differed from the durable finalized snapshot"
            )));
        }
        pre_sum += u128::from(before);
        post_sum += u128::from(after);
        finalized_balances.insert(*key, after);
        let mut expected = before;
        if *key == owner {
            expected = expected
                .checked_sub(
                    intent
                        .position_top_up_lamports
                        .checked_add(intent.admission_top_up_lamports)
                        .ok_or_else(|| Error::new("admission top-up sum overflow"))?,
                )
                .ok_or_else(|| Error::new("admission owner balance underflow"))?;
        }
        if *key == payer {
            expected = expected
                .checked_sub(intent.transaction_fee_lamports)
                .ok_or_else(|| Error::new("admission payer fee underflow"))?;
        }
        if *key == position {
            expected = expected
                .checked_add(intent.position_top_up_lamports)
                .ok_or_else(|| Error::new("admission Position balance overflow"))?;
        }
        if *key == admission {
            expected = expected
                .checked_add(intent.admission_top_up_lamports)
                .ok_or_else(|| Error::new("admission receipt balance overflow"))?;
        }
        if after != expected {
            return Err(Error::new(format!(
                "lamport delta for {key} was not the exact admission fee/rent accounting"
            )));
        }
    }
    if pre_sum.checked_sub(post_sum) != Some(u128::from(intent.transaction_fee_lamports)) {
        return Err(Error::new(
            "whole admission balance vector did not conserve lamports except the exact fee",
        ));
    }
    // A durable input absent from the exact signed message cannot have been
    // mutated by this transaction and remains its prestate projection. Every
    // account that the message did carry was checked above in canonical key
    // order against both historical balance vectors.
    Ok(finalized_balances)
}

fn project_admission_poststate_from_history_v1(
    intent: &IntentV1,
    finalized_balances: &BTreeMap<Pubkey, u64>,
) -> Result<BTreeMap<String, AccountStateV1>> {
    let position = pubkey(&intent.position)?;
    let admission = pubkey(&intent.admission)?;
    let owner = pubkey(&intent.position_owner)?;
    let payer = pubkey(&intent.fee_payer)?;
    let claims_program = pubkey(&intent.expected_receipt_producer)?;
    let expected_position = decode_expected(&intent.expected_position_base64, "Position")?;
    let expected_admission = decode_expected(&intent.expected_admission_base64, "admission")?;
    let admission_receipt = decode_expected(&intent.expected_receipt_base64, "receipt")?;
    let canonical_admission = ProtocolPositionAdmissionV2::decode_receipt(&admission_receipt)
        .and_then(ProtocolPositionAdmissionV2::to_state_bytes)
        .map_err(|error| Error::new(format!("canonical admission receipt: {error:?}")))?;
    if expected_admission != canonical_admission {
        return Err(Error::new(
            "expected admission state was not the exact state projection of the finalized receipt",
        ));
    }
    LiabilityBasisPositionViewV2::decode(&expected_position)
        .map_err(|error| Error::new(format!("expected Claims Position: {error:?}")))?;

    let mut projected = intent.prestate.clone();
    for (label, key, data_owner, bytes) in [
        (
            "claims_position",
            position,
            claims_program,
            expected_position.as_slice(),
        ),
        (
            "claims_admission",
            admission,
            claims_program,
            expected_admission.as_slice(),
        ),
    ] {
        let before = intent
            .prestate
            .get(label)
            .ok_or_else(|| Error::new(format!("admission prestate omitted {label}")))?;
        let lamports = finalized_balances
            .get(&key)
            .copied()
            .ok_or_else(|| Error::new(format!("finalized balances omitted {label}")))?;
        projected.insert(
            label.into(),
            account_state(
                key,
                Some(&RpcAccount {
                    lamports,
                    owner: data_owner,
                    executable: false,
                    rent_epoch: before.rent_epoch,
                    data: bytes.to_vec(),
                }),
            ),
        );
    }
    for (label, key) in [("position_owner", owner), ("fee_payer", payer)] {
        let before = intent
            .prestate
            .get(label)
            .ok_or_else(|| Error::new(format!("admission prestate omitted {label}")))?;
        if before.owner != system_program::ID.to_string()
            || before.executable
            || before.data_len != 0
            || before.data_sha256 != sha256_hex(&[])
        {
            return Err(Error::new(format!(
                "{label} was not the exact System-owned empty wallet profile"
            )));
        }
        let lamports = finalized_balances
            .get(&key)
            .copied()
            .ok_or_else(|| Error::new(format!("finalized balances omitted {label}")))?;
        projected.insert(
            label.into(),
            account_state(
                key,
                Some(&RpcAccount {
                    lamports,
                    owner: system_program::ID,
                    executable: false,
                    rent_epoch: before.rent_epoch,
                    data: Vec::new(),
                }),
            ),
        );
    }
    authenticate_admission_poststate_map(intent, &projected)?;
    Ok(projected)
}

fn authenticate_collateral_balance_vector(
    meta: &Value,
    message: &VersionedMessage,
    intent: &CollateralIntentV1,
) -> Result<()> {
    if message
        .address_table_lookups()
        .is_some_and(|lookups| !lookups.is_empty())
    {
        return Err(Error::new(
            "collateral balance accounting refuses loaded addresses",
        ));
    }
    let pre = meta
        .get("preBalances")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("finalized collateral meta omitted preBalances"))?;
    let post = meta
        .get("postBalances")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("finalized collateral meta omitted postBalances"))?;
    let keys = message.static_account_keys();
    if pre.len() != keys.len() || post.len() != keys.len() {
        return Err(Error::new(
            "collateral balance vector did not match exact static message keys",
        ));
    }
    let fee_payer = pubkey(&intent.prestate["fee_payer"].address)?;
    let participant_token = pubkey(&intent.participant_token_account)?;
    let mut pre_sum = 0_u128;
    let mut post_sum = 0_u128;
    for ((key, before), after) in keys.iter().zip(pre).zip(post) {
        let before = before
            .as_u64()
            .ok_or_else(|| Error::new("preBalances contained a non-u64"))?;
        let after = after
            .as_u64()
            .ok_or_else(|| Error::new("postBalances contained a non-u64"))?;
        pre_sum += u128::from(before);
        post_sum += u128::from(after);
        let expected_after = if *key == fee_payer {
            before
                .checked_sub(intent.total_fee_payer_debit_lamports)
                .ok_or_else(|| Error::new("fee-payer transaction balance underflow"))?
        } else if *key == participant_token && intent.creates_participant_account {
            before
                .checked_add(intent.participant_account_rent_lamports)
                .ok_or_else(|| Error::new("participant rent balance overflow"))?
        } else {
            before
        };
        if after != expected_after {
            return Err(Error::new(format!(
                "lamport delta for {key} was not exact fee/rent-only accounting"
            )));
        }
    }
    if pre_sum.checked_sub(post_sum) != Some(u128::from(intent.transaction_fee_lamports)) {
        return Err(Error::new(
            "whole collateral balance vector did not conserve lamports except the exact fee",
        ));
    }
    Ok(())
}

fn verify_collateral_poststate(
    rpc: &mut Rpc,
    intent: &CollateralIntentV1,
    minimum_slot: u64,
) -> Result<BTreeMap<String, AccountStateV1>> {
    let labels = intent.prestate.keys().cloned().collect::<Vec<_>>();
    let addresses = labels
        .iter()
        .map(|label| parse_state_address(&intent.prestate[label]))
        .collect::<Result<Vec<_>>>()?;
    let (slot, values) = rpc.finalized_accounts(&addresses, minimum_slot)?;
    if slot < minimum_slot {
        return Err(Error::new(
            "collateral poststate snapshot preceded finalized transaction",
        ));
    }
    let mut poststate = BTreeMap::new();
    for ((label, address), value) in labels.iter().zip(addresses).zip(values) {
        poststate.insert(label.clone(), account_state(address, value.as_ref()));
    }
    authenticate_collateral_poststate_map(intent, &poststate)?;
    Ok(poststate)
}

fn authenticate_collateral_poststate_map(
    intent: &CollateralIntentV1,
    poststate: &BTreeMap<String, AccountStateV1>,
) -> Result<()> {
    if poststate.len() != intent.prestate.len() || poststate.keys().ne(intent.prestate.keys()) {
        return Err(Error::new(
            "finalized collateral poststate labels differed from the exact planned account set",
        ));
    }
    let token_program = &intent.token_program;
    let source_expected = decode_expected(&intent.expected_source_base64, "source token")?;
    let participant_expected = decode_expected(
        &intent.expected_participant_token_base64,
        "participant token",
    )?;
    let source_before = &intent.prestate["source_account"];
    let participant_before = &intent.prestate["participant_token_account"];
    require_exact_account_bytes_v1(
        &poststate["source_account"],
        pubkey(&intent.source_account)?,
        &source_expected,
        pubkey(token_program)?,
        source_before.lamports,
        Some(source_before.rent_epoch),
        "collateral source",
    )?;
    let participant_lamports = participant_before
        .lamports
        .checked_add(intent.participant_account_rent_lamports)
        .ok_or_else(|| Error::new("participant token rent overflow"))?;
    require_exact_account_bytes_v1(
        &poststate["participant_token_account"],
        pubkey(&intent.participant_token_account)?,
        &participant_expected,
        pubkey(token_program)?,
        participant_lamports,
        (!intent.creates_participant_account).then_some(participant_before.rent_epoch),
        "participant token",
    )?;
    authenticate_delegated_collateral_token(intent, &participant_expected)?;
    let payer = pubkey(&intent.prestate["fee_payer"].address)?;
    let source_key = pubkey(&intent.source_account)?;
    let participant_key = pubkey(&intent.participant_token_account)?;
    for (label, before) in &intent.prestate {
        let key = parse_state_address(before)?;
        let after = &poststate[label];
        if key == source_key || key == participant_key {
            continue;
        }
        if key == payer {
            let payer_lamports = before
                .lamports
                .checked_sub(intent.total_fee_payer_debit_lamports)
                .ok_or_else(|| Error::new("fee payer poststate underflow"))?;
            require_exact_account_bytes_v1(
                after,
                payer,
                &[],
                system_program::ID,
                payer_lamports,
                Some(before.rent_epoch),
                "collateral fee payer",
            )?;
        } else if after != before {
            return Err(Error::new(format!(
                "collateral input {label} changed unexpectedly"
            )));
        }
    }
    Ok(())
}

fn authenticate_delegated_collateral_token(
    intent: &CollateralIntentV1,
    bytes: &[u8],
) -> Result<()> {
    let participant = TokenAccount::parse(bytes)
        .map_err(|error| Error::new(format!("participant poststate: {error:?}")))?;
    if participant.amount != intent.quantity_atoms
        || participant.mint != pubkey(&intent.mint)?.to_bytes()
        || participant.owner != pubkey(&intent.participant)?.to_bytes()
        || participant.delegate != COption::Some(pubkey(&intent.custody_authority)?.to_bytes())
        || participant.delegated_amount != intent.quantity_atoms
    {
        return Err(Error::new(
            "participant token poststate did not establish exact Custody allowance",
        ));
    }
    Ok(())
}

fn verify_persisted_collateral(rpc: &mut Rpc, report: &ReportV1) -> Result<()> {
    let collateral = report
        .collateral
        .as_ref()
        .ok_or_else(|| Error::new("finalized collateral report disappeared"))?;
    if collateral.phase != CollateralPhaseV1::Finalized {
        return Err(Error::new(
            "persisted collateral history verifier requires finalized phase",
        ));
    }
    let persisted = collateral
        .finalized
        .as_ref()
        .ok_or_else(|| Error::new("finalized collateral report omitted evidence"))?;
    let history = authenticate_collateral_finalized_history(rpc, collateral, &persisted.signature)?;
    if history.signature != persisted.signature
        || history.slot != persisted.slot
        || history.fee_lamports != persisted.fee_lamports
        || history.compute_units_consumed != persisted.compute_units_consumed
        || history.return_data != persisted.return_data
    {
        return Err(Error::new(
            "persisted collateral evidence differed from immutable finalized transaction history",
        ));
    }
    // A later Direct trade legitimately spends the allowance and changes both
    // token accounts. Completion evidence therefore authenticates its exact
    // transaction-time poststate from the deterministic packet, finalized
    // history, and persisted exact bytes; it never reinterprets mutable live
    // accounts as historical state.
    authenticate_collateral_poststate_map(&collateral.intent, &persisted.poststate)
}

fn predict_admission_account_bytes_v1(
    snapshot: &UserPositionAdmissionSnapshotV1,
    unsigned: &UserPositionAdmissionPlanV1,
    position_owner: Pubkey,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let claims_market = LiabilityBasisMarketViewV2::decode(&snapshot.claims_market.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    let position_width = liability_basis_vector_width_v2(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        claims_market.claim_count,
    )
    .map_err(|error| Error::new(format!("Claims Position width: {error:?}")))?;
    let mut expected_position = vec![0; position_width];
    let zero_balances = vec![
        0;
        usize::try_from(claims_market.claim_count).map_err(|_| Error::new(
            "Claims outcome width does not fit host usize"
        ))?
    ];
    encode_liability_basis_position_into_v2(
        LiabilityBasisPositionInputV2 {
            revision: 0,
            market_account: unsigned.claims_request.market,
            owner: position_owner.to_bytes(),
            basis_id: claims_market.basis_id,
        },
        &zero_balances,
        &mut expected_position,
    )
    .map_err(|error| Error::new(format!("predict Claims Position: {error:?}")))?;
    let expected_admission =
        ProtocolPositionAdmissionV2::decode_receipt(&unsigned.expected_receipt_body)
            .and_then(ProtocolPositionAdmissionV2::to_state_bytes)
            .map_err(|error| Error::new(format!("predict Claims admission state: {error:?}")))?;
    Ok((expected_position, expected_admission.to_vec()))
}

fn complete_admission_message_prestate_v1(
    rpc: &mut Rpc,
    message: &VersionedMessage,
    minimum_slot: u64,
    states: &mut BTreeMap<String, AccountStateV1>,
) -> Result<()> {
    let existing = states
        .values()
        .map(parse_state_address)
        .collect::<Result<BTreeSet<_>>>()?;
    let missing = message
        .static_account_keys()
        .iter()
        .enumerate()
        .filter_map(|(index, key)| (!existing.contains(key)).then_some((index, *key)))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    let keys = missing.iter().map(|(_, key)| *key).collect::<Vec<_>>();
    let (_, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
    for ((index, key), value) in missing.into_iter().zip(values) {
        states.insert(
            format!("message_account_{index:03}"),
            account_state(key, value.as_ref()),
        );
    }
    Ok(())
}

fn build_report(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan_sha256: &str,
    evidence_sha256: &str,
    snapshot: &SnapshotBundleV1,
    unsigned: &UserPositionAdmissionPlanV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<ReportV1> {
    if unsigned.required_signer != arguments.position_owner {
        return Err(Error::new("operator selected a substituted Position owner"));
    }
    if snapshot.fee_payer.owner != system_program::ID
        || snapshot.fee_payer.executable
        || !snapshot.fee_payer.data.is_empty()
    {
        return Err(Error::new(
            "fee payer must be one existing System-owned, nonexecutable, data-empty wallet",
        ));
    }
    let (recent_blockhash, last_valid_block_height) = latest_blockhash(rpc)?;
    // The admission's Trading->Claims chain exceeds the 200k default budget;
    // the same house ComputeBudget declarations every ladder message carries.
    let bounded = crate::rpc::bounded_instructions(&unsigned.instructions, None)?;
    let compiled = compile_v0_message_with_optional_tables(
        arguments.fee_payer,
        &bounded,
        recent_blockhash,
        unsigned.observation,
        &snapshot.routing_tables,
    )
    .map_err(|error| Error::new(format!("admission message compilation: {error:?}")))?;
    // The operator built every meta from `UserPositionAdmissionFrameV1`, and
    // the program re-checks that frame against what the RUNTIME presents. Those
    // are not the same thing: compiling into a transaction can promote an
    // account the frame requires readonly. Compare them here, where it costs a
    // refusal, rather than on chain, where it costs a fee and arrives as one
    // undifferentiated `Content`.
    //
    // Only on a FINAL plan. A plan that still owes rent carries its own System
    // transfers, which legitimately make the owner writable; under `--execute`
    // the driver pays that rent in a separate finalized transfer and re-plans,
    // and it is the re-planned message that gets signed. Checking the
    // provisional shape would refuse every preflight of a participant who has
    // not paid rent yet, which is every new one.
    if unsigned
        .position_top_up_lamports
        .checked_add(unsigned.admission_top_up_lamports)
        .ok_or_else(|| Error::new("admission rent top-up overflow"))?
        == 0
    {
        authenticate_compiled_privileges_v1(&compiled.message, &bounded)?;
    }
    let message_bytes = compiled.message.serialize();
    let message_base64 = BASE64.encode(&message_bytes);
    let mut prestate = snapshot.states.clone();
    complete_admission_message_prestate_v1(
        rpc,
        &compiled.message,
        unsigned.observation.slot,
        &mut prestate,
    )?;
    let transaction_fee_lamports = fee_for_message(rpc, &message_base64)?;
    let top_ups = unsigned
        .position_top_up_lamports
        .checked_add(unsigned.admission_top_up_lamports)
        .ok_or_else(|| Error::new("admission rent top-up overflow"))?;
    let (total_owner_debit_lamports, total_fee_payer_debit_lamports) =
        if arguments.position_owner == arguments.fee_payer {
            let total = top_ups
                .checked_add(transaction_fee_lamports)
                .ok_or_else(|| Error::new("combined admission debit overflow"))?;
            (total, total)
        } else {
            (top_ups, transaction_fee_lamports)
        };
    if snapshot.operator.owner.lamports < total_owner_debit_lamports {
        return Err(Error::new(format!(
            "Position owner has {} lamports but exact admission debit is {} (rent top-ups {}, fee {})",
            snapshot.operator.owner.lamports,
            total_owner_debit_lamports,
            top_ups,
            if arguments.position_owner == arguments.fee_payer {
                transaction_fee_lamports
            } else {
                0
            }
        )));
    }
    if arguments.position_owner != arguments.fee_payer
        && snapshot.fee_payer.lamports < total_fee_payer_debit_lamports
    {
        return Err(Error::new(format!(
            "fee payer has {} lamports but exact transaction fee is {}",
            snapshot.fee_payer.lamports, total_fee_payer_debit_lamports
        )));
    }

    let (expected_position, expected_admission) =
        predict_admission_account_bytes_v1(&snapshot.operator, unsigned, arguments.position_owner)?;

    let instructions = unsigned
        .instructions
        .iter()
        .map(instruction_evidence)
        .collect();
    let intent = IntentV1 {
        plan_sha256: plan_sha256.into(),
        campaign_evidence_sha256: evidence_sha256.into(),
        observation_slot: unsigned.observation.slot,
        observation_unix_timestamp: unsigned.observation.unix_timestamp,
        minimum_finalized_slot: arguments.minimum_finalized_slot,
        position_owner: arguments.position_owner.to_string(),
        fee_payer: arguments.fee_payer.to_string(),
        claims_market: snapshot.operator.claims_market.key.to_string(),
        position: unsigned.position.to_string(),
        admission: unsigned.admission.to_string(),
        founding_trading_custody_replay: snapshot.replay.key.to_string(),
        position_rent_principal_lamports: unsigned.position_rent_principal,
        admission_rent_principal_lamports: unsigned.admission_rent_principal,
        position_top_up_lamports: unsigned.position_top_up_lamports,
        admission_top_up_lamports: unsigned.admission_top_up_lamports,
        transaction_fee_lamports,
        total_owner_debit_lamports,
        total_fee_payer_debit_lamports,
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        wire_bytes: compiled.wire_bytes,
        message_sha256: sha256_hex(&message_bytes),
        message_base64,
        claims_request_sha256: hex(&unsigned.claims_request_digest),
        expected_receipt_producer: unsigned.expected_receipt_producer.to_string(),
        expected_receipt_base64: BASE64.encode(unsigned.expected_receipt_body),
        expected_position_base64: BASE64.encode(expected_position),
        expected_admission_base64: BASE64.encode(expected_admission),
        instructions,
        prestate,
    };
    let intent_sha256 = sha256_hex(&serde_json::to_vec(&intent)?);
    Ok(ReportV1 {
        schema: report_schema_v1(expected_cluster).into(),
        cluster: expected_cluster.evidence_label().into(),
        rpc_url: arguments.origin.redacted_url(),
        authorized_mutation: arguments.execute,
        phase: PhaseV1::Planned,
        intent_sha256,
        envelope_sha256: String::new(),
        intent,
        signed_packet_base64: None,
        signed_packet_sha256: None,
        expected_signature: None,
        finalized: None,
        collateral: None,
    })
}

/// A fee payer can never satisfy a readonly meta, so refuse before spending.
///
/// This is the one privilege conflict that no plan, no prefund and no re-plan
/// can resolve: the fee debits the payer, so the message marks it writable
/// whatever the instructions ask for. If the frame declares that same account
/// readonly anywhere, the admission is unlandable and always was.
///
/// It runs on the FIRST plan, before `prefund_admission_rents_v1` -- which is
/// exactly where cohort-11 needed it. On 2026-09-01 the prefund landed
/// (`4qMCqn7f...`), and only then did the admission wall; the rent was already
/// spent on a transaction that could never have worked. This costs one
/// comparison over metas the operator has already built.
fn require_fee_payer_never_declared_readonly_v1(
    declared: &[Instruction],
    fee_payer: Pubkey,
) -> Result<()> {
    for (position, instruction) in declared.iter().enumerate() {
        for (coordinate, meta) in instruction.accounts.iter().enumerate() {
            if meta.pubkey == fee_payer && !meta.is_writable {
                return Err(Error::new(format!(
                    "instruction {position} coordinate {coordinate} declares {} readonly, and \
                     that account is this transaction's FEE PAYER, which the message marks \
                     writable unconditionally because the fee debits it. No plan and no \
                     prefund can satisfy both: name a different --fee-payer",
                    meta.pubkey
                )));
            }
        }
    }
    Ok(())
}

/// Refuse a compiled message that does not present the declared privileges.
/// Refuse a compiled message that does not present the declared privileges.
///
/// Measured against cohort-11 on 2026-09-02: the admission refused
/// `TradingSbfError::Content` (`0x4003`) after 12,233 CU, because the frame
/// requires the Position owner to sign READONLY and the owner was also the fee
/// payer -- and a fee payer is unconditionally writable, since the fee debits
/// it. The two requirements are jointly unsatisfiable, so an admission whose
/// owner pays its own fee can never land, and nothing said so until the chain
/// did. The driver already prefunds rent in a separate transaction to keep the
/// owner readonly (`prefund_admission_rents_v1`); that is the same defect one
/// step earlier and it cannot fix this one, because the fee payer is writable
/// whatever the instructions say.
///
/// The comparison is exact and covers every instruction, not just the outer:
/// the runtime presents one privilege pair per account coordinate and the frame
/// checks all of them.
fn authenticate_compiled_privileges_v1(
    message: &VersionedMessage,
    declared: &[Instruction],
) -> Result<()> {
    let compiled = message.instructions();
    if compiled.len() != declared.len() {
        return Err(Error::new(format!(
            "compiled message carries {} instructions but the plan declared {}",
            compiled.len(),
            declared.len()
        )));
    }
    for (position, (compiled, declared)) in compiled.iter().zip(declared).enumerate() {
        if compiled.accounts.len() != declared.accounts.len() {
            return Err(Error::new(format!(
                "compiled instruction {position} carries {} accounts but the plan declared {}",
                compiled.accounts.len(),
                declared.accounts.len()
            )));
        }
        for (coordinate, (index, meta)) in
            compiled.accounts.iter().zip(&declared.accounts).enumerate()
        {
            let index = usize::from(*index);
            let signer = message.is_signer(index);
            let writable = message.is_maybe_writable(index, None);
            if signer == meta.is_signer && writable == meta.is_writable {
                continue;
            }
            return Err(Error::new(format!(
                "compiled instruction {position} coordinate {coordinate} ({}) is presented \
                 signer={signer} writable={writable} but the plan declared signer={} \
                 writable={}",
                meta.pubkey, meta.is_signer, meta.is_writable,
            )));
        }
    }
    Ok(())
}

fn rebind_admission_observation_v1(
    snapshot: &mut UserPositionAdmissionSnapshotV1,
    observation: Observation,
) {
    for account in [
        &mut snapshot.claims_market,
        &mut snapshot.position,
        &mut snapshot.admission,
        &mut snapshot.linked_basis_raw,
        &mut snapshot.linked_basis_staging,
        &mut snapshot.product_raw,
        &mut snapshot.product_staging,
        &mut snapshot.result_domain_raw,
        &mut snapshot.result_domain_staging,
        &mut snapshot.portfolio_raw,
        &mut snapshot.portfolio_staging,
        &mut snapshot.rent_sysvar,
        &mut snapshot.system_program,
        &mut snapshot.core_market,
        &mut snapshot.activation_cache,
        &mut snapshot.registry_program,
        &mut snapshot.trading_program,
        &mut snapshot.trading_programdata,
        &mut snapshot.claims_program,
        &mut snapshot.claims_programdata,
        &mut snapshot.core_program,
        &mut snapshot.core_programdata,
        &mut snapshot.owner,
        &mut snapshot.rent_credit,
        &mut snapshot.rent_program,
    ] {
        account.observation = observation;
    }
}

fn authenticate_fresh_admission_plan_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &Value,
    report: &ReportV1,
) -> Result<()> {
    let coordinates = evidence_coordinates(evidence)?;
    let mut snapshot = acquire_snapshot(rpc, arguments, plan, coordinates, evidence)?;
    let observation = Observation {
        slot: report.intent.observation_slot,
        unix_timestamp: report.intent.observation_unix_timestamp,
        finality: Finality::Finalized,
    };
    rebind_admission_observation_v1(&mut snapshot.operator, observation);
    // The frozen routing tables rebind exactly like the semantic accounts:
    // their content cannot change (deactivation slot is u64::MAX and the
    // founding froze the extension plan), and the v0 compiler demands the
    // instruction observation on every table it loads through.
    for table in &mut snapshot.routing_tables {
        table.observation = observation;
    }
    let unsigned = plan_user_position_admission_v1(&snapshot.operator).map_err(|error| {
        Error::new(format!(
            "fresh User Position admission reconstruction refused: {error:?}"
        ))
    })?;
    let instructions = unsigned
        .instructions
        .iter()
        .map(instruction_evidence)
        .collect::<Vec<_>>();
    let recent_blockhash = report
        .intent
        .recent_blockhash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("durable admission blockhash: {error}")))?;
    let bounded = crate::rpc::bounded_instructions(&unsigned.instructions, None)?;
    let compiled = compile_v0_message_with_optional_tables(
        arguments.fee_payer,
        &bounded,
        recent_blockhash,
        observation,
        &snapshot.routing_tables,
    )
    .map_err(|error| Error::new(format!("recompile admission message: {error:?}")))?;
    let message = compiled.message.serialize();
    let message_base64 = BASE64.encode(&message);
    complete_admission_message_prestate_v1(
        rpc,
        &compiled.message,
        observation.slot,
        &mut snapshot.states,
    )?;
    if snapshot.states != report.intent.prestate {
        return Err(Error::new(
            "REFUSED: current finalized admission snapshot differed from the durable complete-message prestate",
        ));
    }
    let fresh_fee = fee_for_message(rpc, &message_base64)?;
    let (expected_position, expected_admission) = predict_admission_account_bytes_v1(
        &snapshot.operator,
        &unsigned,
        arguments.position_owner,
    )?;
    let top_ups = unsigned
        .position_top_up_lamports
        .checked_add(unsigned.admission_top_up_lamports)
        .ok_or_else(|| Error::new("fresh admission rent top-up overflow"))?;
    let (owner_debit, payer_debit) = if arguments.position_owner == arguments.fee_payer {
        let combined = top_ups
            .checked_add(fresh_fee)
            .ok_or_else(|| Error::new("fresh combined admission debit overflow"))?;
        (combined, combined)
    } else {
        (top_ups, fresh_fee)
    };
    if snapshot.fee_payer.owner != system_program::ID
        || snapshot.fee_payer.executable
        || !snapshot.fee_payer.data.is_empty()
        || snapshot.operator.owner.lamports < owner_debit
        || (arguments.position_owner != arguments.fee_payer
            && snapshot.fee_payer.lamports < payer_debit)
    {
        return Err(Error::new(
            "fresh admission owner/payer profile or exact balance refused",
        ));
    }
    // A static admission must stay static; a routed one may load only through
    // the exact frozen tables the caller named on the command line. Any other
    // table in the compiled message is a substitution and refuses.
    if let Some(lookups) = compiled.message.address_table_lookups() {
        for lookup in lookups {
            if !arguments.routing_tables.contains(&lookup.account_key) {
                return Err(Error::new(
                    "fresh canonical admission unexpectedly depended on a lookup table",
                ));
            }
        }
    }
    let reconstructed = IntentV1 {
        plan_sha256: report.intent.plan_sha256.clone(),
        campaign_evidence_sha256: report.intent.campaign_evidence_sha256.clone(),
        observation_slot: observation.slot,
        observation_unix_timestamp: observation.unix_timestamp,
        minimum_finalized_slot: arguments.minimum_finalized_slot,
        position_owner: arguments.position_owner.to_string(),
        fee_payer: arguments.fee_payer.to_string(),
        claims_market: snapshot.operator.claims_market.key.to_string(),
        position: unsigned.position.to_string(),
        admission: unsigned.admission.to_string(),
        founding_trading_custody_replay: snapshot.replay.key.to_string(),
        position_rent_principal_lamports: unsigned.position_rent_principal,
        admission_rent_principal_lamports: unsigned.admission_rent_principal,
        position_top_up_lamports: unsigned.position_top_up_lamports,
        admission_top_up_lamports: unsigned.admission_top_up_lamports,
        transaction_fee_lamports: fresh_fee,
        total_owner_debit_lamports: owner_debit,
        total_fee_payer_debit_lamports: payer_debit,
        recent_blockhash: recent_blockhash.to_string(),
        // Solana exposes no reverse blockhash-to-height query. Fresh
        // getFeeForMessage plus the current-height expiry check are the
        // signing gate; retain the initially observed informational bound.
        last_valid_block_height: report.intent.last_valid_block_height,
        wire_bytes: compiled.wire_bytes,
        message_base64,
        message_sha256: sha256_hex(&message),
        claims_request_sha256: hex(&unsigned.claims_request_digest),
        expected_receipt_producer: unsigned.expected_receipt_producer.to_string(),
        expected_receipt_base64: BASE64.encode(unsigned.expected_receipt_body),
        expected_position_base64: BASE64.encode(expected_position),
        expected_admission_base64: BASE64.encode(expected_admission),
        instructions,
        prestate: snapshot.states,
    };
    require_exact_reconstructed_admission_intent_v1(&report.intent, &reconstructed)
}

fn require_exact_reconstructed_admission_intent_v1(
    durable: &IntentV1,
    reconstructed: &IntentV1,
) -> Result<()> {
    if durable != reconstructed {
        return Err(Error::new(
            "durable admission intent differed from the fresh canonical semantic-owner reconstruction",
        ));
    }
    Ok(())
}

fn resume(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &Value,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
) -> Result<()> {
    authenticate_report_phase_envelopes(report)?;
    authenticate_intent_digest(report)?;
    authenticate_genesis_again(rpc, &arguments.origin)?;
    if report.phase == PhaseV1::Finalized {
        verify_persisted_admission_history(rpc, report)?;
        return Ok(());
    }

    match report.phase {
        PhaseV1::Planned => {
            // This second finalized observation and semantic-owner pass occurs
            // on every unsigned resume, including the immediate execution of a
            // newly persisted plan, and always before the first key read.
            authenticate_fresh_admission_plan_v1(rpc, arguments, plan, evidence, report)?;
            if probe_expected_poststate(rpc, report, 0)? {
                return Err(Error::new(
                    "REFUSED: exact admission state exists but this durable plan has no transaction signature; signing would replay and the receipt producer cannot be authenticated",
                ));
            }
            if arguments.execute {
                sign_and_submit(rpc, arguments, report, journal)
            } else {
                Ok(())
            }
        }
        PhaseV1::Dispatching | PhaseV1::SignedNotSubmitted | PhaseV1::Submitted => {
            let signature = report.expected_signature.clone().ok_or_else(|| {
                Error::new("signed/submitted evidence omitted its expected signature")
            })?;
            let signature_state = admission_signature_state_v1(rpc, &signature)?;
            match admission_recovery_v1(report.phase, Some(signature_state))? {
                AdmissionRecoveryV1::ResendIdenticalPacket => {
                    resend_dispatching_packet_v1(rpc, arguments, report, journal, &signature)
                }
                AdmissionRecoveryV1::PollOnly => {
                    wait_admission_signature_finalized_v1(rpc, &signature)?;
                    finalize_admission_journal_v1(rpc, report, journal, &signature)
                }
                AdmissionRecoveryV1::Finalize => {
                    finalize_admission_journal_v1(rpc, report, journal, &signature)
                }
                AdmissionRecoveryV1::RefuseHistoricAmbiguity => {
                    let state = if probe_expected_poststate(rpc, report, 0)? {
                        "the exact expected state exists but immutable finalized transaction history is absent"
                    } else {
                        "the expected state is absent"
                    };
                    Err(Error::new(format!(
                        "REFUSED: historic transaction {signature} is absent and {state}. Its durable phase is {:?}; only the new Dispatching phase authorizes an identical-packet resend",
                        report.phase
                    )))
                }
                other => Err(Error::new(format!(
                    "admission recovery selected impossible signed-packet action {other:?}"
                ))),
            }
        }
        PhaseV1::Finalized => Ok(()),
    }
}

fn sign_and_submit(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
) -> Result<()> {
    require_prestate_unchanged(rpc, report)?;
    authenticate_genesis_again(rpc, &arguments.origin)?;
    enforce_shared_key_path(
        arguments.position_owner,
        &arguments.position_owner_keypair,
        arguments.fee_payer,
        &arguments.fee_payer_keypair,
    )?;
    // The blockhash is the one field of a planned admission that AGES, and it
    // is bound here rather than at plan time because everything between the
    // two costs real wall clock: a finalized re-observation of every semantic
    // account, a fresh fee quote, a poststate probe and a prestate re-read.
    // Measured on devnet 2026-09-02, the chain was producing 6.0 blocks per
    // second, so `MAX_PROCESSING_AGE` of 150 blocks is a TWENTY-FIVE SECOND
    // life -- shorter than that authentication pass -- and cohort-11's
    // admission refused `BlockhashNotFound` twice with its prefund already
    // landed. Rebinding an UNSIGNED plan is not a re-sign: no signature
    // exists yet, so there is nothing to replay, and the rebound message is
    // durable before the first key read exactly as the original was.
    let bound = rebind_unsigned_admission_blockhash_v1(rpc, report, journal)?;

    // This is deliberately the first key-file access in the command. The
    // complete message, fee, rent arithmetic, prestate, and output destination
    // are already durable above.
    let owner = Keypair::new_from_array(campaign::read_keypair_file(
        &arguments.position_owner_keypair,
        "position-owner",
    )?);
    if owner.pubkey() != arguments.position_owner {
        return Err(Error::new(
            "position-owner keypair does not expand to --position-owner",
        ));
    }
    let fee_payer = if arguments.position_owner == arguments.fee_payer
        && arguments.position_owner_keypair == arguments.fee_payer_keypair
    {
        None
    } else {
        let payer = Keypair::new_from_array(campaign::read_keypair_file(
            &arguments.fee_payer_keypair,
            "fee-payer",
        )?);
        if payer.pubkey() != arguments.fee_payer {
            return Err(Error::new(
                "fee-payer keypair does not expand to --fee-payer",
            ));
        }
        Some(payer)
    };
    // The bytes signed are the freshly bound ones AND the durable ones; the
    // journal was written from this very message, so a divergence here is a
    // journal that does not describe what is about to be signed.
    let message_bytes = bound.0;
    if sha256_hex(&message_bytes) != report.intent.message_sha256 {
        return Err(Error::new("persisted message digest changed"));
    }
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("persisted versioned message: {error}")))?;
    let mut signers: Vec<&dyn Signer> = Vec::new();
    if let Some(payer) = fee_payer.as_ref() {
        signers.push(payer);
        signers.push(&owner);
    } else {
        signers.push(&owner);
    }
    let transaction = VersionedTransaction::try_new(message, &signers)
        .map_err(|error| Error::new(format!("sign admission transaction: {error}")))?;
    let signature = transaction
        .signatures
        .first()
        .copied()
        .ok_or_else(|| Error::new("signed transaction omitted payer signature"))?;
    let wire = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize signed admission: {error}")))?;
    if wire.len() != report.intent.wire_bytes {
        return Err(Error::new(format!(
            "signed admission wire is {} bytes but the durable plan committed {}",
            wire.len(),
            report.intent.wire_bytes
        )));
    }
    report.signed_packet_base64 = Some(BASE64.encode(&wire));
    report.signed_packet_sha256 = Some(sha256_hex(&wire));
    report.expected_signature = Some(signature.to_string());
    report.phase = PhaseV1::Dispatching;
    journal.persist(report)?;

    // Re-enter through the exact durable recovery surface even on the first
    // invocation. It polls the fixed signature before it can authorize one
    // send of the already-signed packet, so a process crash never re-opens a
    // key or signing path.
    let observed = admission_signature_state_v1(rpc, &signature.to_string())?;
    if admission_recovery_v1(report.phase, Some(observed))?
        != AdmissionRecoveryV1::ResendIdenticalPacket
    {
        return match observed {
            AdmissionSignatureStateV1::Pending => {
                wait_admission_signature_finalized_v1(rpc, &signature.to_string())?;
                finalize_admission_journal_v1(rpc, report, journal, &signature.to_string())
            }
            AdmissionSignatureStateV1::Finalized => {
                finalize_admission_journal_v1(rpc, report, journal, &signature.to_string())
            }
            AdmissionSignatureStateV1::Absent => Err(Error::new(
                "fresh Dispatching admission did not authorize its exact first send",
            )),
        };
    }
    resend_dispatching_packet_v1(rpc, arguments, report, journal, &signature.to_string())
}

/// Bind the durable UNSIGNED plan onto a fresh finalized blockhash.
///
/// A blockhash is not a fact about the market: it is the submission parameter
/// that decides which banks will still accept the packet, and it is the only
/// field of a planned admission with an expiry. Every other field is a
/// finalized observation that this driver deliberately spends many round trips
/// re-authenticating; the blockhash is the one that cannot survive them.
///
/// The replay wall is the phase, not the blockhash: only a `Planned` report
/// with no signed packet and no expected signature may be rebound. Once a
/// signature exists, the packet is evidence and the driver refuses to make a
/// second one -- `resend_dispatching_packet_v1` re-sends the exact bytes or
/// refuses, and an expired signed packet is archived rather than re-signed.
fn rebind_unsigned_admission_blockhash_v1(
    rpc: &mut Rpc,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
) -> Result<FreshlyBoundMessageV1> {
    require_rebindable_unsigned_admission_v1(report)?;
    let (recent_blockhash, last_valid_block_height) = latest_blockhash(rpc)?;
    rebind_intent_blockhash_v1(
        &mut report.intent,
        recent_blockhash,
        last_valid_block_height,
    )?;
    report.intent_sha256 = sha256_hex(&serde_json::to_vec(&report.intent)?);
    journal.persist(report)?;
    Ok(FreshlyBoundMessageV1(
        BASE64
            .decode(&report.intent.message_base64)
            .map_err(|error| Error::new(format!("rebound message base64: {error}")))?,
    ))
}

/// The serialized bytes of a message bound onto a blockhash read moments ago.
///
/// `sign_and_submit` takes its message from one of these and from nothing
/// else, so the ordering this module was repaired for is a type rather than a
/// comment: a message compiled at plan time and then aged through the
/// authentication pass has no way to reach a signature. The only constructor
/// is `rebind_unsigned_admission_blockhash_v1`.
struct FreshlyBoundMessageV1(Vec<u8>);

/// The replay wall the rebind stands behind.
///
/// `Planned` with no signed packet and no expected signature is the only shape
/// in which no signature over these bytes exists anywhere, and therefore the
/// only shape in which replacing them can replay nothing.
fn require_rebindable_unsigned_admission_v1(report: &ReportV1) -> Result<()> {
    if report.phase != PhaseV1::Planned
        || report.signed_packet_base64.is_some()
        || report.signed_packet_sha256.is_some()
        || report.expected_signature.is_some()
    {
        return Err(Error::new(
            "REFUSED: only an unsigned Planned admission may be bound onto a fresh blockhash; a signed packet is submission evidence and is never re-signed",
        ));
    }
    Ok(())
}

/// The message surgery the rebind performs, with no network in it.
///
/// A blockhash is a fixed thirty-two bytes in a fixed position of a compiled
/// message, so a rebound message is the same length and the same wire cost as
/// the one it replaces. Both of those are checked rather than assumed: a
/// length change would mean the durable `message_base64` was not the message
/// the durable digest and `wire_bytes` describe.
fn rebind_intent_blockhash_v1(
    intent: &mut IntentV1,
    recent_blockhash: Hash,
    last_valid_block_height: u64,
) -> Result<()> {
    let message_bytes = BASE64
        .decode(&intent.message_base64)
        .map_err(|error| Error::new(format!("persisted message base64: {error}")))?;
    if sha256_hex(&message_bytes) != intent.message_sha256 {
        return Err(Error::new("persisted message digest changed"));
    }
    let mut message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("persisted versioned message: {error}")))?;
    message.set_recent_blockhash(recent_blockhash);
    if message.recent_blockhash() != &recent_blockhash {
        return Err(Error::new(
            "rebound admission message did not carry the fresh blockhash",
        ));
    }
    let rebound = message.serialize();
    if rebound.len() != message_bytes.len() {
        return Err(Error::new(format!(
            "rebinding the admission blockhash changed the message from {} to {} bytes",
            message_bytes.len(),
            rebound.len()
        )));
    }
    intent.recent_blockhash = recent_blockhash.to_string();
    intent.last_valid_block_height = last_valid_block_height;
    intent.message_sha256 = sha256_hex(&rebound);
    intent.message_base64 = BASE64.encode(&rebound);
    Ok(())
}

fn resend_dispatching_packet_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
    signature: &str,
) -> Result<()> {
    if admission_recovery_v1(report.phase, Some(AdmissionSignatureStateV1::Absent))?
        != AdmissionRecoveryV1::ResendIdenticalPacket
    {
        return Err(Error::new(
            "admission send permission requires an absent signature in durable Dispatching",
        ));
    }
    let transaction = authenticate_admission_packet(report)?;
    let expected_signature = signature
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("Dispatching admission signature: {error}")))?;
    if transaction.signatures.first() != Some(&expected_signature) {
        return Err(Error::new(
            "Dispatching admission packet did not carry its durable signature",
        ));
    }
    let wire = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize durable admission packet: {error}")))?;
    authenticate_genesis_again(rpc, &arguments.origin)?;
    require_prestate_unchanged(rpc, report)?;
    // A SIGNED packet cannot be rebound -- that would be a re-sign, and the
    // refusal below is the wall. What it can have is the measurement taken
    // LAST: the genesis and prestate reads above used to sit between this
    // check and the send, and at 6 blocks per second two round trips are
    // twelve blocks of a hundred-and-fifty-block life.
    let height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if height > report.intent.last_valid_block_height {
        return Err(Error::new(
            "Dispatching admission packet expired while absent; archive the journal rather than re-signing",
        ));
    }
    let landed_fault_armed = chaos_fault::is_armed_for_v1(
        POSITION_ADMISSION_CHAOS_MUTATION_V1,
        BoundaryV1::LandedBeforeFinalizationFsync,
    )?;
    if landed_fault_armed && report.cluster != ExpectedClusterV1::OwnedLoopback.evidence_label() {
        return Err(Error::new(
            "position-admission chaos faults are admitted only on owned-loopback",
        ));
    }
    park_position_admission_chaos_boundary_v1(report, journal, BoundaryV1::DispatchingBeforeSend)?;
    let returned = rpc
        .call_once(
            "sendTransaction",
            &json!([BASE64.encode(&wire), {
                "encoding":"base64",
                "skipPreflight":false,
                "preflightCommitment":"finalized",
                "maxRetries":0
            }]),
        )?
        .as_str()
        .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("sendTransaction signature: {error}")))?;
    if returned != expected_signature {
        return Err(Error::new(
            "RPC returned a signature different from the locally signed packet",
        ));
    }
    if landed_fault_armed {
        wait_admission_signature_finalized_v1(rpc, signature)?;
        park_position_admission_chaos_boundary_v1(
            report,
            journal,
            BoundaryV1::LandedBeforeFinalizationFsync,
        )?;
    }
    report.phase = PhaseV1::Submitted;
    journal.persist(report)?;
    wait_admission_signature_finalized_v1(rpc, signature)?;
    finalize_admission_journal_v1(rpc, report, journal, signature)
}

fn finalize_admission_journal_v1(
    rpc: &mut Rpc,
    report: &mut ReportV1,
    journal: &mut ReportJournalV1,
    signature: &str,
) -> Result<()> {
    if report.phase == PhaseV1::Dispatching {
        // A finalized packet recovered from Dispatching still passes through
        // Submitted locally, preserving the adjacent durable grammar without
        // another network send.
        park_position_admission_chaos_boundary_v1(
            report,
            journal,
            BoundaryV1::LandedBeforeFinalizationFsync,
        )?;
        report.phase = PhaseV1::Submitted;
        journal.persist(report)?;
    }
    finalize_known_signature(rpc, report, signature)?;
    journal.persist(report)
}

fn park_position_admission_chaos_boundary_v1(
    report: &ReportV1,
    journal: &ReportJournalV1,
    boundary: BoundaryV1,
) -> Result<()> {
    if !chaos_fault::is_armed_for_v1(POSITION_ADMISSION_CHAOS_MUTATION_V1, boundary)? {
        return Ok(());
    }
    if report.phase != PhaseV1::Dispatching {
        return Err(Error::new(
            "position-admission chaos seam requires a durable Dispatching journal",
        ));
    }
    authenticate_report_phase_envelopes(report)?;
    chaos_fault::park_if_armed_v1(
        &report.cluster,
        POSITION_ADMISSION_CHAOS_MUTATION_V1,
        boundary,
        &journal.path,
        &report.intent_sha256,
        report
            .signed_packet_sha256
            .as_deref()
            .ok_or_else(|| Error::new("admission chaos seam omitted packet digest"))?,
        report
            .expected_signature
            .as_deref()
            .ok_or_else(|| Error::new("admission chaos seam omitted signature"))?,
    )
}

fn finalize_known_signature(rpc: &mut Rpc, report: &mut ReportV1, signature: &str) -> Result<()> {
    let history = authenticate_admission_finalized_history(rpc, report, signature)?;
    report.finalized = Some(FinalizedEvidenceV1 {
        signature: history.signature,
        slot: history.slot,
        fee_lamports: history.fee_lamports,
        compute_units_consumed: history.compute_units_consumed,
        return_data_producer: history.return_data_producer,
        return_data_sha256: history.return_data_sha256,
        poststate: history.poststate,
    });
    report.phase = PhaseV1::Finalized;
    Ok(())
}

fn authenticate_admission_finalized_history(
    rpc: &mut Rpc,
    report: &ReportV1,
    signature: &str,
) -> Result<AdmissionHistoryV1> {
    authenticate_intent_digest(report)?;
    let packet = authenticate_admission_packet(report)?;
    if report.expected_signature.as_deref() != Some(signature) {
        return Err(Error::new(
            "finalized admission signature differs from the durable packet",
        ));
    }
    let transaction = finalized_transaction(rpc, signature)?.ok_or_else(|| {
        Error::new(format!(
            "signature {signature} has not reached finalized history; confirmed is never accepted"
        ))
    })?;
    if transaction
        .pointer("/transaction/signatures/0")
        .and_then(Value::as_str)
        != Some(signature)
    {
        return Err(Error::new(
            "finalized transaction history omitted the exact admission signature",
        ));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized admission transaction omitted slot"))?;
    let (wire_slot, history_wire) = finalized_transaction_wire(rpc, signature)?;
    let expected_wire = report
        .signed_packet_base64
        .as_deref()
        .ok_or_else(|| Error::new("finalized admission journal omitted signed packet"))?;
    require_exact_history_wire_v1(expected_wire, &history_wire)?;
    if wire_slot != slot {
        return Err(Error::new(
            "finalized admission JSON and exact-wire observations disagreed on slot",
        ));
    }
    if slot < report.intent.observation_slot || slot < report.intent.minimum_finalized_slot {
        return Err(Error::new(
            "finalized admission transaction preceded its durable finalized observation",
        ));
    }
    let meta = transaction
        .get("meta")
        .ok_or_else(|| Error::new("finalized admission transaction omitted meta"))?;
    if meta.get("err").is_none_or(|value| !value.is_null()) {
        return Err(Error::new(format!(
            "finalized admission transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized admission transaction omitted fee"))?;
    if fee != report.intent.transaction_fee_lamports {
        return Err(Error::new(format!(
            "finalized fee {fee} differs from planned exact fee {}",
            report.intent.transaction_fee_lamports
        )));
    }
    let return_data = meta
        .get("returnData")
        .ok_or_else(|| Error::new("finalized admission transaction omitted returnData"))?;
    let producer = return_data
        .get("programId")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("finalized returnData omitted producer"))?;
    if producer != report.intent.expected_receipt_producer {
        return Err(Error::new(format!(
            "finalized return data producer {producer} was not Claims {}",
            report.intent.expected_receipt_producer
        )));
    }
    let receipt = decode_exact_base64_tuple_v1(
        return_data
            .get("data")
            .ok_or_else(|| Error::new("finalized returnData omitted data tuple"))?,
        "finalized returnData",
    )?;
    let expected = BASE64
        .decode(&report.intent.expected_receipt_base64)
        .map_err(|error| Error::new(format!("persisted receipt base64: {error}")))?;
    if receipt != expected {
        return Err(Error::new(
            "finalized Claims receipt differed from the semantic owner's prediction",
        ));
    }
    let finalized_balances =
        authenticate_admission_balance_vector(meta, &packet.message, &report.intent)?;
    let poststate =
        project_admission_poststate_from_history_v1(&report.intent, &finalized_balances)?;
    Ok(AdmissionHistoryV1 {
        signature: signature.into(),
        slot,
        fee_lamports: fee,
        compute_units_consumed: meta.get("computeUnitsConsumed").and_then(Value::as_u64),
        return_data_producer: producer.into(),
        return_data_sha256: sha256_hex(&receipt),
        poststate,
    })
}

fn authenticate_admission_poststate_map(
    intent: &IntentV1,
    post: &BTreeMap<String, AccountStateV1>,
) -> Result<()> {
    if post.len() != intent.prestate.len() || post.keys().ne(intent.prestate.keys()) {
        return Err(Error::new(
            "finalized admission poststate labels differed from the exact planned account set",
        ));
    }
    let expected_position = decode_expected(&intent.expected_position_base64, "Position")?;
    let expected_admission = decode_expected(&intent.expected_admission_base64, "admission")?;
    let position = post
        .get("claims_position")
        .ok_or_else(|| Error::new("poststate omitted Claims Position"))?;
    let admission = post
        .get("claims_admission")
        .ok_or_else(|| Error::new("poststate omitted Claims admission"))?;
    require_exact_account_bytes_v1(
        position,
        pubkey(&intent.position)?,
        &expected_position,
        pubkey(&intent.expected_receipt_producer)?,
        intent.position_rent_principal_lamports,
        None,
        "Claims Position",
    )?;
    LiabilityBasisPositionViewV2::decode(&expected_position)
        .map_err(|error| Error::new(format!("expected Claims Position: {error:?}")))?;
    require_exact_account_bytes_v1(
        admission,
        pubkey(&intent.admission)?,
        &expected_admission,
        pubkey(&intent.expected_receipt_producer)?,
        intent.admission_rent_principal_lamports,
        None,
        "Claims admission",
    )?;
    ProtocolPositionAdmissionV2::decode(&expected_admission)
        .map_err(|error| Error::new(format!("expected Claims admission: {error:?}")))?;

    for label in [
        "claims_market",
        "core_market",
        "lifecycle_rent_credit",
        "founding_trading_custody_replay",
    ] {
        if post.get(label) != intent.prestate.get(label) {
            return Err(Error::new(format!(
                "{label} changed during User Position admission; this route owns no such mutation"
            )));
        }
    }
    // Every authenticated release/record/sysvar input is immutable across this
    // route. Owner and payer lamports plus the two newly allocated accounts are
    // the only permitted changes.
    for (label, before) in &intent.prestate {
        if matches!(
            label.as_str(),
            "claims_position" | "claims_admission" | "position_owner" | "fee_payer"
        ) {
            continue;
        }
        if post.get(label) != Some(before) {
            return Err(Error::new(format!(
                "authenticated admission input {label} changed unexpectedly"
            )));
        }
    }
    verify_wallet_debits(intent, post)
}

fn verify_wallet_debits(intent: &IntentV1, post: &BTreeMap<String, AccountStateV1>) -> Result<()> {
    let owner_before = &intent.prestate["position_owner"];
    let payer_before = &intent.prestate["fee_payer"];
    let owner_after = &post["position_owner"];
    let payer_after = &post["fee_payer"];
    if !same_wallet_except_lamports(owner_before, owner_after)
        || !same_wallet_except_lamports(payer_before, payer_after)
    {
        return Err(Error::new(
            "Position owner or fee payer changed owner, privilege, rent epoch, width, or data during admission",
        ));
    }
    if intent.position_owner == intent.fee_payer {
        let expected = owner_before
            .lamports
            .checked_sub(intent.total_owner_debit_lamports)
            .ok_or_else(|| Error::new("owner post-balance underflow"))?;
        if owner_after.lamports != expected || payer_after != owner_after {
            return Err(Error::new(
                "combined owner/fee-payer debit differed from exact rent+fee",
            ));
        }
    } else {
        let expected_owner = owner_before
            .lamports
            .checked_sub(intent.total_owner_debit_lamports)
            .ok_or_else(|| Error::new("owner post-balance underflow"))?;
        let expected_payer = payer_before
            .lamports
            .checked_sub(intent.total_fee_payer_debit_lamports)
            .ok_or_else(|| Error::new("fee-payer post-balance underflow"))?;
        if owner_after.lamports != expected_owner || payer_after.lamports != expected_payer {
            return Err(Error::new(
                "owner or fee-payer debit differed from exact rent/fee split",
            ));
        }
    }
    Ok(())
}

fn same_wallet_except_lamports(before: &AccountStateV1, after: &AccountStateV1) -> bool {
    before.address == after.address
        && before.owner == after.owner
        && before.executable == after.executable
        && before.rent_epoch == after.rent_epoch
        && before.data_len == after.data_len
        && before.data_sha256 == after.data_sha256
}

fn probe_expected_poststate(rpc: &mut Rpc, report: &ReportV1, floor: u64) -> Result<bool> {
    let position = Pubkey::from_str(&report.intent.position)
        .map_err(|error| Error::new(format!("persisted Position: {error}")))?;
    let admission = Pubkey::from_str(&report.intent.admission)
        .map_err(|error| Error::new(format!("persisted admission: {error}")))?;
    let (_, values) = rpc.finalized_accounts(&[position, admission], floor)?;
    let expected_position = decode_expected(&report.intent.expected_position_base64, "Position")?;
    let expected_admission =
        decode_expected(&report.intent.expected_admission_base64, "admission")?;
    match (&values[0], &values[1]) {
        (None, None) => Ok(false),
        (Some(position), Some(admission)) => {
            // A prefunded pair - System-owned, data-empty, holding only the
            // rent the separate prefund transfer paid - is the admitted
            // route's own designed prestate, not an occupation: the founding
            // pre-funds its allocated accounts the same way.
            if position.owner == system_program::ID
                && position.data.is_empty()
                && !position.executable
                && admission.owner == system_program::ID
                && admission.data.is_empty()
                && !admission.executable
            {
                return Ok(false);
            }
            let exact = position.owner.to_string() == report.intent.expected_receipt_producer
                && position.lamports == report.intent.position_rent_principal_lamports
                && position.data == expected_position
                && admission.owner.to_string() == report.intent.expected_receipt_producer
                && admission.lamports == report.intent.admission_rent_principal_lamports
                && admission.data == expected_admission;
            if exact {
                Ok(true)
            } else {
                Err(Error::new(
                    "REFUSED: canonical Position/admission coordinates are occupied by another state; never replay or overwrite",
                ))
            }
        }
        _ => Err(Error::new(
            "REFUSED: only one of Position/admission exists; the admission transaction is atomic, so this is a conflicting state",
        )),
    }
}

fn require_prestate_unchanged(rpc: &mut Rpc, report: &ReportV1) -> Result<()> {
    let labels = report.intent.prestate.keys().cloned().collect::<Vec<_>>();
    let addresses = labels
        .iter()
        .map(|label| parse_state_address(&report.intent.prestate[label]))
        .collect::<Result<Vec<_>>>()?;
    let (_, values) = rpc.finalized_accounts(&addresses, report.intent.observation_slot)?;
    for ((label, address), value) in labels.iter().zip(addresses).zip(values) {
        let now = account_state(address, value.as_ref());
        if now != report.intent.prestate[label] {
            return Err(Error::new(format!(
                "REFUSED: {label} changed after the durable finalized plan; this may be a stale deployment, release graph, wallet balance, or replay"
            )));
        }
    }
    Ok(())
}

fn acquire_snapshot(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    coordinates: CoordinatesV1,
    evidence: &Value,
) -> Result<SnapshotBundleV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let raw_keys = [
        coordinates.linked_basis,
        coordinates.product,
        coordinates.result_domain,
        coordinates.portfolio,
    ];
    let (raw_slot, raw_values) =
        rpc.finalized_accounts(&raw_keys, arguments.minimum_finalized_slot)?;
    let schemas = [
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        PORTFOLIO_SCHEMA_ID_V2,
    ];
    let mut records = Vec::new();
    for ((raw, schema), value) in raw_keys.into_iter().zip(schemas).zip(raw_values) {
        let account = value.ok_or_else(|| Error::new(format!("missing finalized record {raw}")))?;
        let digest: [u8; 32] = Sha256::digest(&account.data).into();
        let expected_raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            &registry,
        )
        .0;
        if raw != expected_raw {
            return Err(Error::new(format!(
                "campaign evidence record {raw} is not the PDA of its finalized bytes"
            )));
        }
        let staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            &registry,
        )
        .0;
        records.push(RecordCoordinatesV1 { raw, staging });
    }

    let claims = pubkey(&plan.claims.program_id)?;
    let position_seeds = dclutch_claims_svm::protocol_position_v2::ProtocolPositionSeedsV2::new(
        coordinates.claims_market.to_bytes(),
        arguments.position_owner.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Position seeds: {error:?}")))?;
    let admission_seeds =
        dclutch_claims_svm::protocol_position_v2::ProtocolPositionAdmissionSeedsV2::new(
            coordinates.claims_market.to_bytes(),
            arguments.position_owner.to_bytes(),
        )
        .map_err(|error| Error::new(format!("admission seeds: {error:?}")))?;
    let position = Pubkey::find_program_address(&position_seeds.as_slices(), &claims).0;
    let admission = Pubkey::find_program_address(&admission_seeds.as_slices(), &claims).0;
    let keys = vec![
        ("claims_market", coordinates.claims_market),
        ("claims_position", position),
        ("claims_admission", admission),
        ("linked_basis_raw", records[0].raw),
        ("linked_basis_staging", records[0].staging),
        ("product_raw", records[1].raw),
        ("product_staging", records[1].staging),
        ("result_domain_raw", records[2].raw),
        ("result_domain_staging", records[2].staging),
        ("portfolio_raw", records[3].raw),
        ("portfolio_staging", records[3].staging),
        ("rent_sysvar", sysvar::rent::ID),
        ("system_program", system_program::ID),
        ("core_market", coordinates.core_market),
        ("activation_cache", pubkey(&plan.activation)?),
        ("registry_program", registry),
        ("trading_program", pubkey(&plan.trading.program_id)?),
        ("trading_programdata", pubkey(&plan.trading.programdata_id)?),
        ("claims_program", claims),
        ("claims_programdata", pubkey(&plan.claims.programdata_id)?),
        ("core_program", pubkey(&plan.core.program_id)?),
        ("core_programdata", pubkey(&plan.core.programdata_id)?),
        ("position_owner", arguments.position_owner),
        ("lifecycle_rent_credit", coordinates.rent_credit),
        ("rent_program", pubkey(&plan.rent_credit.program_id)?),
        (
            "founding_trading_custody_replay",
            coordinates.trading_replay,
        ),
        ("fee_payer", arguments.fee_payer),
    ];
    let mut unique = Vec::new();
    for (_, key) in &keys {
        if !unique.contains(key) {
            unique.push(*key);
        }
    }
    let semantic_len = unique.len();
    for table in &arguments.routing_tables {
        if unique.contains(table) {
            return Err(Error::new(
                "--routing-table aliased a semantic admission account",
            ));
        }
        unique.push(*table);
    }
    let (slot, values) = rpc.finalized_accounts(&unique, raw_slot)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    let mut unique = unique;
    let mut values = values;
    let table_values = values.split_off(semantic_len);
    let table_keys = unique.split_off(semantic_len);
    let by_key = unique.into_iter().zip(values).collect::<BTreeMap<_, _>>();
    let observed = |key: Pubkey, allow_vacant: bool| -> Result<ObservedAccount> {
        match by_key
            .get(&key)
            .ok_or_else(|| Error::new(format!("snapshot omitted requested account {key}")))?
        {
            Some(account) => Ok(to_observed(observation, key, account)),
            None if allow_vacant => Ok(ObservedAccount {
                observation,
                key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: Vec::new(),
            }),
            None => Err(Error::new(format!(
                "snapshot missing required account {key}"
            ))),
        }
    };
    let position_account = observed(position, true)?;
    let admission_account = observed(admission, true)?;
    let operator = UserPositionAdmissionSnapshotV1 {
        genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
        claims_market: observed(coordinates.claims_market, false)?,
        position: position_account,
        admission: admission_account,
        linked_basis_raw: observed(records[0].raw, false)?,
        linked_basis_staging: observed(records[0].staging, true)?,
        product_raw: observed(records[1].raw, false)?,
        product_staging: observed(records[1].staging, true)?,
        result_domain_raw: observed(records[2].raw, false)?,
        result_domain_staging: observed(records[2].staging, true)?,
        portfolio_raw: observed(records[3].raw, false)?,
        portfolio_staging: observed(records[3].staging, true)?,
        rent_sysvar: observed(sysvar::rent::ID, false)?,
        system_program: observed(system_program::ID, false)?,
        core_market: observed(coordinates.core_market, false)?,
        activation_cache: observed(pubkey(&plan.activation)?, false)?,
        registry_program: observed(registry, false)?,
        trading_program: observed(pubkey(&plan.trading.program_id)?, false)?,
        trading_programdata: observed(pubkey(&plan.trading.programdata_id)?, false)?,
        claims_program: observed(claims, false)?,
        claims_programdata: observed(pubkey(&plan.claims.programdata_id)?, false)?,
        core_program: observed(pubkey(&plan.core.program_id)?, false)?,
        core_programdata: observed(pubkey(&plan.core.programdata_id)?, false)?,
        owner: observed(arguments.position_owner, false)?,
        rent_credit: observed(coordinates.rent_credit, false)?,
        rent_program: observed(pubkey(&plan.rent_credit.program_id)?, false)?,
    };
    let replay = observed(coordinates.trading_replay, false)?;
    let fee_payer = observed(arguments.fee_payer, false)?;
    authenticate_evidence_hints(evidence, &operator, &replay)?;
    let mut states = BTreeMap::new();
    for (label, key) in keys {
        let value = by_key.get(&key).and_then(Option::as_ref);
        states.insert(label.into(), account_state(key, value));
    }
    let routing_tables = table_keys
        .into_iter()
        .zip(table_values)
        .map(|(key, value)| {
            let value =
                value.ok_or_else(|| Error::new(format!("snapshot missing routing table {key}")))?;
            Ok(ObservedAccount {
                observation,
                key,
                owner: value.owner,
                lamports: value.lamports,
                executable: value.executable,
                data: value.data,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SnapshotBundleV1 {
        operator,
        replay,
        fee_payer,
        states,
        routing_tables,
    })
}

fn evidence_coordinates(evidence: &Value) -> Result<CoordinatesV1> {
    let rent_credit = evidence_address(evidence, "founding_lifecycle_rent_credit")?;
    if rent_credit == evidence_address(evidence, "lifecycle_rent_credit")? {
        return Err(Error::new(
            "campaign aliased the DCLTGMF3 founding rent credit to the earlier Found37 generation",
        ));
    }
    Ok(CoordinatesV1 {
        claims_market: evidence_address(evidence, "claims_aggregate")?,
        core_market: evidence_address(evidence, "founding_market")?,
        rent_credit,
        trading_replay: evidence_address(evidence, "founding_normal_custody_replay")?,
        product: evidence_address(evidence, "product_record")?,
        result_domain: evidence_address(evidence, "result_domain_record")?,
        portfolio: evidence_address(evidence, "portfolio_record")?,
        linked_basis: evidence_address(evidence, "linked_liability_basis_record")?,
    })
}

fn evidence_accounts(evidence: &Value) -> Result<&serde_json::Map<String, Value>> {
    evidence
        .pointer("/execution/market/accounts")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("campaign evidence omitted execution.market.accounts"))
}

fn evidence_address(evidence: &Value, label: &str) -> Result<Pubkey> {
    let value = evidence_accounts(evidence)?
        .get(label)
        .and_then(|value| value.get("address"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("campaign evidence omitted {label}.address")))?;
    pubkey(value)
}

fn authenticate_plan_evidence(plan_sha256: &str, evidence: &Value) -> Result<()> {
    if evidence.get("schema").and_then(Value::as_str)
        != Some("dclutch-successor-campaign-report-v1")
        || evidence.get("plan_sha256").and_then(Value::as_str) != Some(plan_sha256)
        || evidence
            .pointer("/execution/completed")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(Error::new(
            "campaign evidence schema, plan digest, or completed execution refused",
        ));
    }
    Ok(())
}

fn authenticate_evidence_hints(
    evidence: &Value,
    snapshot: &UserPositionAdmissionSnapshotV1,
    replay: &ObservedAccount,
) -> Result<()> {
    for (label, account) in [
        ("claims_aggregate", &snapshot.claims_market),
        ("founding_market", &snapshot.core_market),
        ("founding_lifecycle_rent_credit", &snapshot.rent_credit),
        ("founding_normal_custody_replay", replay),
        ("product_record", &snapshot.product_raw),
        ("result_domain_record", &snapshot.result_domain_raw),
        ("portfolio_record", &snapshot.portfolio_raw),
        ("linked_liability_basis_record", &snapshot.linked_basis_raw),
    ] {
        let row = evidence_accounts(evidence)?
            .get(label)
            .ok_or_else(|| Error::new(format!("campaign evidence omitted {label}")))?;
        if row.get("address").and_then(Value::as_str) != Some(&account.key.to_string()) {
            return Err(Error::new(format!("campaign evidence substituted {label}")));
        }
        // Immutable finalized records must still be the exact founding bytes.
        if label.ends_with("_record")
            && row.get("data_sha256").and_then(Value::as_str) != Some(&sha256_hex(&account.data))
        {
            return Err(Error::new(format!("finalized record {label} changed")));
        }
    }
    Ok(())
}

fn authenticate_existing_report(
    report: &ReportV1,
    arguments: &ArgumentsV1,
    plan_sha256: &str,
    evidence_sha256: &str,
    plan: &SuccessorPlan,
    evidence: &Value,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    authenticate_report_phase_envelopes(report)?;
    if report.schema != report_schema_v1(expected_cluster)
        || report.cluster != expected_cluster.evidence_label()
        || report.intent.plan_sha256 != plan_sha256
        || report.intent.campaign_evidence_sha256 != evidence_sha256
        || report.intent.position_owner != arguments.position_owner.to_string()
        || report.intent.fee_payer != arguments.fee_payer.to_string()
        || report.intent.minimum_finalized_slot != arguments.minimum_finalized_slot
    {
        return Err(Error::new(
            "existing output belongs to another plan, evidence file, owner, payer, or finalized floor",
        ));
    }
    authenticate_intent_digest(report)?;
    match (&arguments.collateral, &report.collateral) {
        (None, Some(_)) => Err(Error::new(
            "existing output includes collateral preparation; its explicit source/key/quantity arguments are required to resume",
        )),
        (Some(arguments), Some(_)) => {
            authenticate_collateral_arguments(report, arguments, plan, evidence)
        }
        _ => Ok(()),
    }
}

fn report_schema_v1(expected_cluster: ExpectedClusterV1) -> &'static str {
    match expected_cluster {
        ExpectedClusterV1::Devnet => REPORT_SCHEMA_V1,
        ExpectedClusterV1::OwnedLoopback => LOCAL_REPORT_SCHEMA_V1,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnvelopePhaseV1 {
    Planned,
    SignedNotSubmitted,
    Dispatching,
    Submitted,
    Finalized,
}

fn validate_phase_fields(
    label: &str,
    phase: EnvelopePhaseV1,
    packet: Option<&str>,
    packet_digest: Option<&str>,
    signature: Option<&str>,
    finalized_signature: Option<&str>,
) -> Result<()> {
    let valid = match phase {
        EnvelopePhaseV1::Planned => {
            packet.is_none()
                && packet_digest.is_none()
                && signature.is_none()
                && finalized_signature.is_none()
        }
        EnvelopePhaseV1::SignedNotSubmitted
        | EnvelopePhaseV1::Dispatching
        | EnvelopePhaseV1::Submitted => {
            packet.is_some()
                && packet_digest.is_some()
                && signature.is_some()
                && finalized_signature.is_none()
        }
        EnvelopePhaseV1::Finalized => {
            packet.is_some()
                && packet_digest.is_some()
                && signature.is_some()
                && finalized_signature.is_some()
                && signature == finalized_signature
        }
    };
    if !valid {
        return Err(Error::new(format!(
            "{label} phase did not have its exact packet/signature/finalized envelope"
        )));
    }
    Ok(())
}

fn validate_report_phase_shape(report: &ReportV1) -> Result<()> {
    let phase = match report.phase {
        PhaseV1::Planned => EnvelopePhaseV1::Planned,
        PhaseV1::SignedNotSubmitted => EnvelopePhaseV1::SignedNotSubmitted,
        PhaseV1::Dispatching => EnvelopePhaseV1::Dispatching,
        PhaseV1::Submitted => EnvelopePhaseV1::Submitted,
        PhaseV1::Finalized => EnvelopePhaseV1::Finalized,
    };
    validate_phase_fields(
        "admission",
        phase,
        report.signed_packet_base64.as_deref(),
        report.signed_packet_sha256.as_deref(),
        report.expected_signature.as_deref(),
        report
            .finalized
            .as_ref()
            .map(|value| value.signature.as_str()),
    )?;
    if phase != EnvelopePhaseV1::Finalized && report.collateral.is_some() {
        return Err(Error::new(
            "collateral journal cannot exist before finalized admission",
        ));
    }
    if phase != EnvelopePhaseV1::Planned {
        authenticate_admission_packet(report)?;
    }
    if let Some(collateral) = &report.collateral {
        let phase = match collateral.phase {
            CollateralPhaseV1::Planned => EnvelopePhaseV1::Planned,
            CollateralPhaseV1::SignedNotSubmitted => EnvelopePhaseV1::SignedNotSubmitted,
            CollateralPhaseV1::Submitted => EnvelopePhaseV1::Submitted,
            CollateralPhaseV1::Finalized => EnvelopePhaseV1::Finalized,
        };
        validate_phase_fields(
            "collateral",
            phase,
            collateral.signed_packet_base64.as_deref(),
            collateral.signed_packet_sha256.as_deref(),
            collateral.expected_signature.as_deref(),
            collateral
                .finalized
                .as_ref()
                .map(|value| value.signature.as_str()),
        )?;
        if phase != EnvelopePhaseV1::Planned {
            authenticate_collateral_packet(collateral)?;
        }
    }
    Ok(())
}

fn admission_envelope_digest(report: &ReportV1) -> Result<String> {
    Ok(sha256_hex(&serde_json::to_vec(&(
        report.phase,
        &report.signed_packet_base64,
        &report.signed_packet_sha256,
        &report.expected_signature,
        &report.finalized,
    ))?))
}

fn collateral_envelope_digest(report: &CollateralReportV1) -> Result<String> {
    Ok(sha256_hex(&serde_json::to_vec(&(
        report.phase,
        &report.signed_packet_base64,
        &report.signed_packet_sha256,
        &report.expected_signature,
        &report.finalized,
    ))?))
}

fn refresh_report_envelope_digests(report: &mut ReportV1) -> Result<()> {
    if let Some(collateral) = report.collateral.as_mut() {
        collateral.envelope_sha256 = collateral_envelope_digest(collateral)?;
    }
    report.envelope_sha256 = admission_envelope_digest(report)?;
    Ok(())
}

fn authenticate_report_phase_envelopes(report: &ReportV1) -> Result<()> {
    validate_report_phase_shape(report)?;
    if report.envelope_sha256 != admission_envelope_digest(report)? {
        return Err(Error::new(
            "durable admission phase envelope digest changed",
        ));
    }
    if let Some(collateral) = &report.collateral
        && collateral.envelope_sha256 != collateral_envelope_digest(collateral)?
    {
        return Err(Error::new(
            "durable collateral phase envelope digest changed",
        ));
    }
    Ok(())
}

fn authenticate_intent_digest(report: &ReportV1) -> Result<()> {
    if sha256_hex(&serde_json::to_vec(&report.intent)?) != report.intent_sha256 {
        return Err(Error::new("durable admission intent digest changed"));
    }
    Ok(())
}

fn verify_persisted_admission_history(rpc: &mut Rpc, report: &ReportV1) -> Result<()> {
    let evidence = report
        .finalized
        .as_ref()
        .ok_or_else(|| Error::new("finalized admission omitted evidence"))?;
    let history = authenticate_admission_finalized_history(rpc, report, &evidence.signature)?;
    if history.signature != evidence.signature
        || history.slot != evidence.slot
        || history.fee_lamports != evidence.fee_lamports
        || history.compute_units_consumed != evidence.compute_units_consumed
        || history.return_data_producer != evidence.return_data_producer
        || history.return_data_sha256 != evidence.return_data_sha256
        || history.poststate != evidence.poststate
    {
        return Err(Error::new(
            "persisted admission evidence differed from immutable finalized transaction history",
        ));
    }
    // Claims Positions are mutable after admission. The exact transaction-time
    // projection is re-derived from immutable history above; a later live
    // Position is deliberately never consulted.
    authenticate_admission_poststate_map(&report.intent, &evidence.poststate)
}

fn admission_signature_state_v1(
    rpc: &mut Rpc,
    signature: &str,
) -> Result<AdmissionSignatureStateV1> {
    signature
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("durable admission signature: {error}")))?;
    let response = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let values = response
        .get("value")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("admission signature status omitted value array"))?;
    if values.len() != 1 {
        return Err(Error::new(
            "admission signature status did not return exactly one row",
        ));
    }
    let status = &values[0];
    if status.is_null() {
        return Ok(AdmissionSignatureStateV1::Absent);
    }
    let error = status
        .get("err")
        .ok_or_else(|| Error::new("admission signature status omitted err"))?;
    if !error.is_null() {
        return Err(Error::new(format!(
            "admission transaction {signature} failed before finalization: {error}"
        )));
    }
    match status.get("confirmationStatus").and_then(Value::as_str) {
        Some("processed" | "confirmed") => Ok(AdmissionSignatureStateV1::Pending),
        Some("finalized") => {
            let transaction = rpc.call(
                "getTransaction",
                &json!([signature, {
                    "encoding":"json",
                    "commitment":"finalized",
                    "maxSupportedTransactionVersion":0
                }]),
            )?;
            if transaction.is_null() {
                return Err(Error::new(
                    "finalized admission signature omitted finalized transaction history",
                ));
            }
            Ok(AdmissionSignatureStateV1::Finalized)
        }
        _ => Err(Error::new(
            "admission signature status had an unknown confirmation state",
        )),
    }
}

fn wait_admission_signature_finalized_v1(rpc: &mut Rpc, signature: &str) -> Result<()> {
    let deadline = Instant::now() + FINALITY_WAIT;
    while Instant::now() < deadline {
        if admission_signature_state_v1(rpc, signature)? == AdmissionSignatureStateV1::Finalized {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(Error::new(format!(
        "transaction {signature} did not reach finalized history within 300 seconds; durable packet retained for poll-only resume"
    )))
}

fn finalized_transaction(rpc: &mut Rpc, signature: &str) -> Result<Option<Value>> {
    let status = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let status = status
        .get("value")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .filter(|value| !value.is_null());
    let Some(status) = status else {
        return Ok(None);
    };
    if status.get("confirmationStatus").and_then(Value::as_str) != Some("finalized") {
        return Ok(None);
    }
    let transaction = rpc.call(
        "getTransaction",
        &json!([signature, {
            "encoding":"json",
            "commitment":"finalized",
            "maxSupportedTransactionVersion":0
        }]),
    )?;
    if transaction.is_null() {
        return Err(Error::new(
            "finalized signature omitted finalized transaction history",
        ));
    }
    Ok(Some(transaction))
}

/// Pay the Position and admission rents from the owner in one finalized
/// transaction of its own, so the admission message never debits the owner.
fn prefund_admission_rents_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    unsigned: &UserPositionAdmissionPlanV1,
) -> Result<()> {
    let owner = read_expected_keypair(
        &arguments.position_owner_keypair,
        arguments.position_owner,
        "prefund owner",
    )?;
    let mut instructions = Vec::new();
    if unsigned.position_top_up_lamports != 0 {
        instructions.push(solana_system_interface::instruction::transfer(
            &arguments.position_owner,
            &unsigned.position,
            unsigned.position_top_up_lamports,
        ));
    }
    if unsigned.admission_top_up_lamports != 0 {
        instructions.push(solana_system_interface::instruction::transfer(
            &arguments.position_owner,
            &unsigned.admission,
            unsigned.admission_top_up_lamports,
        ));
    }
    let (recent_blockhash, _) = latest_blockhash(rpc)?;
    let mut signers: Vec<&dyn Signer> = vec![&owner];
    let payer;
    let payer_key = if arguments.fee_payer == arguments.position_owner {
        arguments.position_owner
    } else {
        payer = read_expected_keypair(
            &arguments.fee_payer_keypair,
            arguments.fee_payer,
            "prefund fee payer",
        )?;
        signers.insert(0, &payer);
        arguments.fee_payer
    };
    let message = Message::new_with_blockhash(&instructions, Some(&payer_key), &recent_blockhash);
    let transaction = Transaction::new(&signers, message, recent_blockhash);
    let wire = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize prefund: {error}")))?;
    let returned = rpc.call_once(
        "sendTransaction",
        &json!([BASE64.encode(&wire), {"encoding": "base64", "preflightCommitment": "finalized"}]),
    )?;
    let signature = returned
        .as_str()
        .ok_or_else(|| Error::new("prefund sendTransaction result was not a signature"))?;
    eprintln!("admission prefund transfer submitted: {signature}");
    wait_finalized(rpc, signature)?;
    Ok(())
}

fn wait_finalized(rpc: &mut Rpc, signature: &str) -> Result<()> {
    let deadline = Instant::now() + FINALITY_WAIT;
    while Instant::now() < deadline {
        if finalized_transaction(rpc, signature)?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(Error::new(format!(
        "transaction {signature} did not reach finalized history within 300 seconds; durable signature retained for resume"
    )))
}

fn latest_blockhash(rpc: &mut Rpc) -> Result<(Hash, u64)> {
    let value = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = value
        .get("value")
        .ok_or_else(|| Error::new("getLatestBlockhash omitted value"))?;
    let hash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("latest blockhash: {error}")))?;
    let last_valid = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted lastValidBlockHeight"))?;
    Ok((hash, last_valid))
}

fn fee_for_message(rpc: &mut Rpc, message_base64: &str) -> Result<u64> {
    rpc.call(
        "getFeeForMessage",
        &json!([message_base64, {"commitment":"finalized"}]),
    )?
    .get("value")
    .and_then(Value::as_u64)
    .ok_or_else(|| Error::new("getFeeForMessage omitted exact fee"))
}

fn authenticate_genesis_again(rpc: &mut Rpc, origin: &ClusterOriginV1) -> Result<()> {
    let genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("getGenesisHash result was not a string"))?
        .to_owned();
    origin.authenticate_genesis(&genesis)
}

fn to_observed(observation: Observation, key: Pubkey, account: &RpcAccount) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data.clone(),
    }
}

fn account_state(address: Pubkey, account: Option<&RpcAccount>) -> AccountStateV1 {
    let (owner, lamports, executable, rent_epoch, data): (Pubkey, u64, bool, u64, &[u8]) =
        match account {
            Some(account) => (
                account.owner,
                account.lamports,
                account.executable,
                account.rent_epoch,
                &account.data,
            ),
            None => (system_program::ID, 0, false, 0, &[]),
        };
    let mut exact = Sha256::new();
    exact.update(owner.as_ref());
    exact.update(lamports.to_le_bytes());
    exact.update([u8::from(executable)]);
    exact.update(rent_epoch.to_le_bytes());
    exact.update(u64::try_from(data.len()).unwrap_or(u64::MAX).to_le_bytes());
    exact.update(data);
    AccountStateV1 {
        address: address.to_string(),
        owner: owner.to_string(),
        lamports,
        executable,
        rent_epoch,
        data_len: data.len(),
        data_sha256: sha256_hex(data),
        account_sha256: hex(&exact.finalize()),
    }
}

fn require_account_bytes(
    state: &AccountStateV1,
    bytes: &[u8],
    owner: &str,
    lamports: u64,
    label: &str,
) -> Result<()> {
    if state.owner != owner
        || state.executable
        || state.lamports != lamports
        || state.data_len != bytes.len()
        || state.data_sha256 != sha256_hex(bytes)
    {
        return Err(Error::new(format!(
            "{label} owner, privilege, exact rent, width, or bytes differed from prediction"
        )));
    }
    Ok(())
}

fn require_exact_account_bytes_v1(
    state: &AccountStateV1,
    address: Pubkey,
    bytes: &[u8],
    owner: Pubkey,
    lamports: u64,
    expected_rent_epoch: Option<u64>,
    label: &str,
) -> Result<()> {
    let exact = RpcAccount {
        lamports,
        owner,
        executable: false,
        rent_epoch: state.rent_epoch,
        data: bytes.to_vec(),
    };
    if parse_state_address(state)? != address
        || state.owner != owner.to_string()
        || state.executable
        || state.lamports != lamports
        || expected_rent_epoch.is_some_and(|expected| state.rent_epoch != expected)
        || account_state(address, Some(&exact)) != *state
    {
        return Err(Error::new(format!(
            "{label} address, owner, privilege, exact rent, rent epoch, width, bytes, or account digest differed from prediction"
        )));
    }
    Ok(())
}

fn parse_state_address(state: &AccountStateV1) -> Result<Pubkey> {
    Pubkey::from_str(&state.address)
        .map_err(|error| Error::new(format!("persisted account address: {error}")))
}

fn decode_expected(value: &str, label: &str) -> Result<Vec<u8>> {
    BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("persisted {label} base64: {error}")))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

impl ReportJournalV1 {
    fn vacant(path: PathBuf) -> Self {
        Self {
            path,
            expected_bytes: None,
        }
    }

    fn existing(path: PathBuf, expected_bytes: Vec<u8>) -> Self {
        Self {
            path,
            expected_bytes: Some(expected_bytes),
        }
    }

    fn persist(&mut self, report: &mut ReportV1) -> Result<()> {
        validate_report_phase_shape(report)?;
        refresh_report_envelope_digests(report)?;
        authenticate_report_phase_envelopes(report)?;
        let mut bytes = serde_json::to_vec_pretty(report)?;
        bytes.push(b'\n');
        self.persist_exact_bytes(bytes)
    }

    fn persist_exact_bytes(&mut self, bytes: Vec<u8>) -> Result<()> {
        if !self.path.is_absolute() {
            return Err(Error::new("admission output path must be absolute"));
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::new("admission output omitted its parent directory"))?;
        let name = self
            .path
            .file_name()
            .ok_or_else(|| Error::new("admission output omitted its file name"))?;
        let mut lock_name = OsString::from(".");
        lock_name.push(name);
        lock_name.push(".user-position.lock");
        let lock_path = parent.join(lock_name);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| {
                Error::new(format!(
                    "open journal lock {}: {error}",
                    lock_path.display()
                ))
            })?;
        lock.try_lock().map_err(|error| {
            Error::new(format!(
                "REFUSED: another writer holds journal lock {}: {error}",
                lock_path.display()
            ))
        })?;

        if let Some(expected) = self.expected_bytes.as_deref() {
            let current = fs::read(&self.path).map_err(|error| {
                Error::new(format!(
                    "read journal CAS prestate {}: {error}",
                    self.path.display()
                ))
            })?;
            if current != expected {
                return Err(Error::new(format!(
                    "REFUSED: journal {} changed since this process read it; stale writer will not overwrite",
                    self.path.display()
                )));
            }
        }

        let mut temporary = None;
        let mut file = None;
        for nonce in 0_u8..=u8::MAX {
            let mut temporary_name = OsString::from(".");
            temporary_name.push(name);
            temporary_name.push(format!(".user-position-{}-{nonce}.tmp", std::process::id()));
            let candidate = parent.join(temporary_name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(opened) => {
                    temporary = Some(candidate);
                    file = Some(opened);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(Error::new(format!(
                        "create journal temporary {}: {error}",
                        candidate.display()
                    )));
                }
            }
        }
        let temporary = temporary.ok_or_else(|| {
            Error::new("all bounded journal temporary names were already occupied")
        })?;
        let mut file = file.ok_or_else(|| Error::new("journal temporary file disappeared"))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);

        let placement = if self.expected_bytes.is_none() {
            fs::hard_link(&temporary, &self.path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    Error::new(format!(
                        "REFUSED: admission output {} was concurrently created; no file was overwritten",
                        self.path.display()
                    ))
                } else {
                    Error::new(format!(
                        "no-replace link {} from {}: {error}",
                        self.path.display(),
                        temporary.display()
                    ))
                }
            })
        } else {
            // The sidecar OS lock serializes every cooperating updater, and
            // the exact-current-bytes comparison above is the CAS predicate.
            fs::rename(&temporary, &self.path).map_err(|error| {
                Error::new(format!(
                    "CAS replace {} from {}: {error}",
                    self.path.display(),
                    temporary.display()
                ))
            })
        };
        if let Err(error) = placement {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        let directory = OpenOptions::new()
            .read(true)
            .open(parent)
            .map_err(|error| Error::new(format!("open output directory: {error}")))?;
        directory.sync_all()?;
        if temporary.exists() {
            fs::remove_file(&temporary).map_err(|error| {
                Error::new(format!(
                    "remove linked journal temporary {}: {error}",
                    temporary.display()
                ))
            })?;
            directory.sync_all()?;
        }
        self.expected_bytes = Some(bytes);
        Ok(())
    }
}

fn print_report(report: &ReportV1) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(report)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut owner = None;
    let mut owner_keypair = None;
    let mut payer = None;
    let mut payer_keypair = None;
    let mut minimum_slot = None;
    let mut output = None;
    let mut execute = false;
    let mut collateral_source_owner = None;
    let mut collateral_source_owner_keypair = None;
    let mut collateral_source_account = None;
    let mut collateral_quantity_atoms = None;
    let mut routing_tables_raw: Option<String> = None;
    let mut seen = BTreeSet::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(Error::new("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        if !seen.insert(argument.clone()) {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        match argument.as_str() {
            "--rpc-url" => rpc_url = Some(value),
            DEVNET_ACKNOWLEDGMENT_FLAG => acknowledgment = Some(value),
            "--plan" => plan = Some(value),
            "--campaign-evidence" => evidence = Some(value),
            "--position-owner" => owner = Some(value),
            "--position-owner-keypair" => owner_keypair = Some(value),
            "--fee-payer" => payer = Some(value),
            "--fee-payer-keypair" => payer_keypair = Some(value),
            "--minimum-finalized-slot" => minimum_slot = Some(value),
            "--output" => output = Some(value),
            "--collateral-source-owner" => collateral_source_owner = Some(value),
            "--collateral-source-owner-keypair" => collateral_source_owner_keypair = Some(value),
            "--collateral-source-account" => collateral_source_account = Some(value),
            "--collateral-quantity-atoms" => collateral_quantity_atoms = Some(value),
            "--routing-table" => routing_tables_raw = Some(value),
            _ => {
                return Err(Error::new(format!(
                    "unknown devnet-user-position-admission-v1 argument: {argument}"
                )));
            }
        }
    }
    let required = |value: Option<String>, label: &str| {
        value.ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let absolute = |value: Option<String>, label: &str| -> Result<PathBuf> {
        let path = PathBuf::from(required(value, label)?);
        if !path.is_absolute() {
            return Err(Error::new(format!("{label} must be absolute")));
        }
        Ok(path)
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    let minimum_finalized_slot = required(minimum_slot, "--minimum-finalized-slot")?
        .parse::<u64>()
        .map_err(|_| Error::new("--minimum-finalized-slot must be a decimal u64"))?;
    if minimum_finalized_slot == 0 {
        return Err(Error::new("--minimum-finalized-slot must be nonzero"));
    }
    let collateral = match (
        collateral_source_owner,
        collateral_source_owner_keypair,
        collateral_source_account,
        collateral_quantity_atoms,
    ) {
        (None, None, None, None) => None,
        (Some(owner), Some(owner_keypair), Some(account), Some(quantity)) => {
            let quantity_atoms = quantity
                .parse::<u64>()
                .map_err(|_| Error::new("--collateral-quantity-atoms must be a decimal u64"))?;
            if quantity_atoms == 0 {
                return Err(Error::new("--collateral-quantity-atoms must be positive"));
            }
            Some(CollateralArgumentsV1 {
                source_owner: pubkey(&owner)?,
                source_owner_keypair: absolute(
                    Some(owner_keypair),
                    "--collateral-source-owner-keypair",
                )?,
                source_account: pubkey(&account)?,
                quantity_atoms,
            })
        }
        _ => {
            return Err(Error::new(
                "collateral preparation requires all four of --collateral-source-owner, --collateral-source-owner-keypair, --collateral-source-account, and --collateral-quantity-atoms",
            ));
        }
    };
    let routing_tables = match routing_tables_raw {
        None => Vec::new(),
        Some(raw) => {
            let mut tables = Vec::new();
            for part in raw.split(',') {
                let table = pubkey(part.trim())?;
                if tables.contains(&table) {
                    return Err(Error::new("--routing-table repeats a table"));
                }
                tables.push(table);
            }
            tables
        }
    };
    Ok(ArgumentsV1 {
        origin,
        plan: absolute(plan, "--plan")?,
        campaign_evidence: absolute(evidence, "--campaign-evidence")?,
        position_owner: pubkey(&required(owner, "--position-owner")?)?,
        position_owner_keypair: absolute(owner_keypair, "--position-owner-keypair")?,
        fee_payer: pubkey(&required(payer, "--fee-payer")?)?,
        fee_payer_keypair: absolute(payer_keypair, "--fee-payer-keypair")?,
        minimum_finalized_slot,
        output: absolute(output, "--output")?,
        execute,
        collateral,
        routing_tables,
    })
}

pub(crate) fn usage() -> &'static str {
    concat!(
        "Usage:\n  dclutch-local-successor-bootstrap devnet-user-position-admission-v1 \\\n",
        "     --rpc-url URL --i-mean-devnet DEVNET_GENESIS_HASH --plan ABSOLUTE_JSON \\\n",
        "     --campaign-evidence ABSOLUTE_JSON --position-owner PUBKEY \\\n",
        "     --position-owner-keypair ABSOLUTE_JSON --fee-payer PUBKEY \\\n",
        "     --fee-payer-keypair ABSOLUTE_JSON --minimum-finalized-slot U64 \\\n",
        "     --output ABSOLUTE_JSON [--execute] \\\n",
        "     [--collateral-source-owner PUBKEY \\\n",
        "      --collateral-source-owner-keypair ABSOLUTE_JSON \\\n",
        "      --collateral-source-account PUBKEY --collateral-quantity-atoms U64]\n\n",
        "Default is finalized read-only planning. The complete canonical admission message, exact rent top-ups, exact getFeeForMessage fee, input fingerprints, and output path are fsynced before --execute reads a key file. When the complete collateral tuple is present, a finalized admission is followed by a separately journaled Token-2022 transfer into the chain-derived participant account and an exact Custody allowance; that second complete packet, raw-atom quantity, account rent, fee, and token pre/post bytes are also fsynced before its first key read. The command only admits Solana devnet and never accepts confirmed state. It never re-signs a persisted admission: Dispatching recovery polls the exact signature first and may resend only the identical packet when the signature is absent; a present or Submitted packet is poll-only through finalized history."
    )
}

pub(crate) fn local_usage() -> &'static str {
    concat!(
        "Usage:\n  dclutch-local-successor-bootstrap local-private-validator-user-position-admission-v1 \\\n",
        "     --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON \\\n",
        "     --campaign-evidence ABSOLUTE_JSON --position-owner PUBKEY \\\n",
        "     --position-owner-keypair ABSOLUTE_JSON --fee-payer PUBKEY \\\n",
        "     --fee-payer-keypair ABSOLUTE_JSON --minimum-finalized-slot U64 \\\n",
        "     --output ABSOLUTE_JSON [--execute] \\\n",
        "     [--collateral-source-owner PUBKEY \\\n",
        "      --collateral-source-owner-keypair ABSOLUTE_JSON \\\n",
        "      --collateral-source-account PUBKEY --collateral-quantity-atoms U64]\n\n",
        "This command is exclusively for a supervisor-owned loopback validator. It invokes the same admission and collateral semantic owners as the public devnet command, but writes the distinct dclutch-owned-loopback-user-position-admission-execution-v1 report and refuses every external RPC origin. The report is also the supervisor's crash journal: its exact packet, signature, and Dispatching phase are fsynced before the position-admission pre-send fault seam."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::DEVNET_GENESIS_HASH;
    use dclutch_claims_svm::protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionAdmissionEvidenceV2, ProtocolPositionOwnerKindV2,
        ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
    };
    use dclutch_core_contract::ContentId;
    use dclutch_market_core_codec::{Identity, MarketIdentity, Readiness};
    use dclutch_realm_contract::RealmV1Input;
    use dclutch_registry_contract::{
        ArtifactActivationInputV1, ArtifactUpgradePolicyV1, ExecutionReleaseActivationInputsV1,
        activate_execution_release_set_v1,
    };
    use dclutch_release_set_contract::{
        ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
    };
    use dclutch_token_svm::MINT_BYTES;
    use solana_program::hash::hash;

    fn base_args() -> Vec<String> {
        vec![
            "--rpc-url".into(),
            "https://api.devnet.solana.com".into(),
            "--i-mean-devnet".into(),
            DEVNET_GENESIS_HASH.into(),
            "--plan".into(),
            "/private/tmp/plan.json".into(),
            "--campaign-evidence".into(),
            "/private/tmp/evidence.json".into(),
            "--position-owner".into(),
            Pubkey::new_unique().to_string(),
            "--position-owner-keypair".into(),
            "/private/tmp/owner.json".into(),
            "--fee-payer".into(),
            Pubkey::new_unique().to_string(),
            "--fee-payer-keypair".into(),
            "/private/tmp/payer.json".into(),
            "--minimum-finalized-slot".into(),
            "91".into(),
            "--output".into(),
            "/private/tmp/output.json".into(),
        ]
    }

    #[test]
    fn cli_is_explicit_and_defaults_to_no_mutation() {
        let parsed = parse_arguments(base_args()).expect("explicit CLI");
        assert!(!parsed.execute);
        assert_eq!(parsed.minimum_finalized_slot, 91);
        assert!(parsed.collateral.is_none());
    }

    #[test]
    fn collateral_cli_requires_the_complete_positive_raw_atom_tuple() {
        let mut partial = base_args();
        partial.extend([
            "--collateral-source-owner".into(),
            Pubkey::new_unique().to_string(),
        ]);
        assert!(
            parse_arguments(partial)
                .expect_err("partial")
                .to_string()
                .contains("requires all four")
        );

        let mut complete = base_args();
        complete.extend([
            "--collateral-source-owner".into(),
            Pubkey::new_unique().to_string(),
            "--collateral-source-owner-keypair".into(),
            "/private/tmp/source.json".into(),
            "--collateral-source-account".into(),
            Pubkey::new_unique().to_string(),
            "--collateral-quantity-atoms".into(),
            "0".into(),
        ]);
        assert!(
            parse_arguments(complete)
                .expect_err("zero")
                .to_string()
                .contains("must be positive")
        );

        let mut maximum = base_args();
        let maximum_owner = Pubkey::new_unique();
        let maximum_source = Pubkey::new_unique();
        maximum.extend([
            "--collateral-source-owner".into(),
            maximum_owner.to_string(),
            "--collateral-source-owner-keypair".into(),
            "/private/tmp/source.json".into(),
            "--collateral-source-account".into(),
            maximum_source.to_string(),
            "--collateral-quantity-atoms".into(),
            u64::MAX.to_string(),
        ]);
        let parsed = parse_arguments(maximum).expect("exact u64 raw atoms");
        let parsed = parsed.collateral.expect("collateral tuple");
        assert_eq!(parsed.source_owner, maximum_owner);
        assert_eq!(parsed.source_account, maximum_source);
        assert_eq!(parsed.quantity_atoms, u64::MAX);
    }

    fn collateral_fixture() -> (
        Vec<Instruction>,
        CollateralArgumentsV1,
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
    ) {
        let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        let participant = Pubkey::new_unique();
        let participant_token = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let custody_authority = Pubkey::new_unique();
        let source = Pubkey::new_unique();
        let source_owner = Pubkey::new_unique();
        let collateral = CollateralArgumentsV1 {
            source_owner,
            source_owner_keypair: "/private/tmp/source.json".into(),
            source_account: source,
            quantity_atoms: 0x0807_0605_0403_0201,
        };
        let instructions = vec![
            token_instruction(
                transfer_checked(
                    token_program.to_bytes(),
                    source.to_bytes(),
                    mint.to_bytes(),
                    participant_token.to_bytes(),
                    source_owner.to_bytes(),
                    collateral.quantity_atoms,
                    9,
                )
                .expect("transfer"),
            ),
            token_instruction(
                approve_checked(
                    token_program.to_bytes(),
                    participant_token.to_bytes(),
                    mint.to_bytes(),
                    custody_authority.to_bytes(),
                    participant.to_bytes(),
                    collateral.quantity_atoms,
                    9,
                )
                .expect("approve"),
            ),
        ];
        (
            instructions,
            collateral,
            participant,
            participant_token,
            token_program,
            mint,
            custody_authority,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_fixture(
        instructions: &[Instruction],
        collateral: &CollateralArgumentsV1,
        participant: Pubkey,
        participant_token: Pubkey,
        token_program: Pubkey,
        mint: Pubkey,
        custody_authority: Pubkey,
        decimals: u8,
    ) -> Result<()> {
        authenticate_collateral_instruction_sequence_v1(
            instructions,
            false,
            Pubkey::new_unique(),
            participant,
            participant_token,
            "unused-without-create",
            collateral,
            token_program,
            mint,
            custody_authority,
            decimals,
            0,
        )
    }

    #[test]
    fn collateral_sequence_binds_exact_atoms_mint_decimals_authorities_and_order() {
        let (instructions, collateral, participant, target, token, mint, custody) =
            collateral_fixture();
        authenticate_fixture(
            &instructions,
            &collateral,
            participant,
            target,
            token,
            mint,
            custody,
            9,
        )
        .expect("canonical sequence");

        let mut wrong_quantity = collateral.clone();
        wrong_quantity.quantity_atoms = wrong_quantity.quantity_atoms.saturating_add(1);
        assert!(
            authenticate_fixture(
                &instructions,
                &wrong_quantity,
                participant,
                target,
                token,
                mint,
                custody,
                9,
            )
            .is_err()
        );
        assert!(
            authenticate_fixture(
                &instructions,
                &collateral,
                participant,
                target,
                token,
                Pubkey::new_unique(),
                custody,
                9,
            )
            .is_err()
        );
        assert!(
            authenticate_fixture(
                &instructions,
                &collateral,
                participant,
                target,
                token,
                mint,
                custody,
                8,
            )
            .is_err()
        );
        assert!(
            authenticate_fixture(
                &instructions,
                &collateral,
                participant,
                target,
                token,
                mint,
                Pubkey::new_unique(),
                9,
            )
            .is_err()
        );

        let mut reordered = instructions.clone();
        reordered.swap(0, 1);
        assert!(
            authenticate_fixture(
                &reordered,
                &collateral,
                participant,
                target,
                token,
                mint,
                custody,
                9,
            )
            .is_err()
        );
        let mut substituted_owner = instructions.clone();
        substituted_owner[0].accounts[3].pubkey = Pubkey::new_unique();
        assert!(
            authenticate_fixture(
                &substituted_owner,
                &collateral,
                participant,
                target,
                token,
                mint,
                custody,
                9,
            )
            .is_err()
        );
        let mut substituted_source = collateral.clone();
        substituted_source.source_account = Pubkey::new_unique();
        assert!(
            authenticate_fixture(
                &instructions,
                &substituted_source,
                participant,
                target,
                token,
                mint,
                custody,
                9,
            )
            .is_err()
        );
        assert!(
            authenticate_fixture(
                &instructions,
                &collateral,
                Pubkey::new_unique(),
                target,
                token,
                mint,
                custody,
                9,
            )
            .is_err()
        );
    }

    #[test]
    fn collateral_create_path_binds_payer_base_seed_rent_and_order() {
        let (mut instructions, collateral, participant, target, token, mint, custody) =
            collateral_fixture();
        let fee_payer = Pubkey::new_unique();
        let seed = "0123456789abcdef0123456789abcdef";
        let rent = 2_039_280;
        instructions.insert(
            0,
            create_account_with_seed(
                &fee_payer,
                &target,
                &participant,
                seed,
                rent,
                u64::try_from(ACCOUNT_BYTES).expect("width"),
                &token,
            ),
        );
        instructions.insert(
            1,
            token_instruction(
                initialize_account3(
                    token.to_bytes(),
                    target.to_bytes(),
                    mint.to_bytes(),
                    participant.to_bytes(),
                )
                .expect("initialize"),
            ),
        );
        let authenticate = |payer, base, candidate_seed: &str, candidate_rent| {
            authenticate_collateral_instruction_sequence_v1(
                &instructions,
                true,
                payer,
                base,
                target,
                candidate_seed,
                &collateral,
                token,
                mint,
                custody,
                9,
                candidate_rent,
            )
        };
        authenticate(fee_payer, participant, seed, rent).expect("canonical create path");
        assert!(authenticate(Pubkey::new_unique(), participant, seed, rent).is_err());
        assert!(authenticate(fee_payer, Pubkey::new_unique(), seed, rent).is_err());
        assert!(authenticate(fee_payer, participant, "different", rent).is_err());
        assert!(authenticate(fee_payer, participant, seed, rent.saturating_add(1)).is_err());
        let mut reordered = instructions.clone();
        reordered.swap(0, 1);
        assert!(
            authenticate_collateral_instruction_sequence_v1(
                &reordered,
                true,
                fee_payer,
                participant,
                target,
                seed,
                &collateral,
                token,
                mint,
                custody,
                9,
                rent,
            )
            .is_err()
        );
    }

    fn account_state_fixture(address: Pubkey, lamports: u64) -> AccountStateV1 {
        AccountStateV1 {
            address: address.to_string(),
            owner: system_program::ID.to_string(),
            lamports,
            executable: false,
            rent_epoch: 0,
            data_len: 0,
            data_sha256: sha256_hex(&[]),
            account_sha256: sha256_hex(address.as_ref()),
        }
    }

    fn exact_account_state_fixture(
        address: Pubkey,
        lamports: u64,
        owner: Pubkey,
        executable: bool,
        data: Vec<u8>,
    ) -> AccountStateV1 {
        account_state(
            address,
            Some(&RpcAccount {
                lamports,
                owner,
                executable,
                rent_epoch: 0,
                data,
            }),
        )
    }

    fn admission_exactness_fixture() -> (ReportV1, VersionedTransaction) {
        let position_owner = Keypair::new();
        admission_exactness_fixture_for(&position_owner)
    }

    fn admission_exactness_fixture_for(
        position_owner: &Keypair,
    ) -> (ReportV1, VersionedTransaction) {
        let fee_payer = Keypair::new();
        let position = Pubkey::new_unique();
        let admission = Pubkey::new_unique();
        let claims_program = Pubkey::new_unique();
        let claims_market = Pubkey::new_unique();
        let logical_market = Pubkey::new_unique();
        let rent_credit = Pubkey::new_unique();
        let rent_program = Pubkey::new_unique();
        let trading_replay = Pubkey::new_unique();
        let request = ProtocolPositionRequestV2::new(ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind: ProtocolPositionOwnerKindV2::User,
            presence: ProtocolPositionPresenceV2::Vacant,
            release_set: [0x11; 32],
            market: logical_market.to_bytes(),
            position_owner: position_owner.pubkey().to_bytes(),
            parent_request_digest: [0x12; 32],
            rent_credit: rent_credit.to_bytes(),
            rent_program: rent_program.to_bytes(),
            generation: 3,
            expected_market_revision: 7,
            expected_position_revision: 0,
            observed_position_lamports: 100,
            observed_admission_lamports: 200,
            position_rent_principal: 100,
            admission_rent_principal: 200,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        })
        .expect("admission request");
        let request_bytes = request.to_bytes().expect("request bytes");
        let request_digest = hash(&request_bytes).to_bytes();
        let admitted = ProtocolPositionAdmissionV2::new(
            request,
            ProtocolPositionAdmissionEvidenceV2 {
                product_record_digest: [0x21; 32],
                semantic_basis_id: [0x22; 32],
                linked_basis_record_digest: [0x23; 32],
                request_digest,
                claims_program: claims_program.to_bytes(),
                trading_program: [0x24; 32],
                capability_descriptor: [0; 32],
                capability_outcome: 0,
                outcome_count: 2,
            },
        )
        .expect("admission state");
        let expected_admission = admitted.to_state_bytes().expect("admission state bytes");
        let expected_receipt = admitted
            .to_receipt_bytes()
            .expect("admission receipt bytes");
        let mut expected_position =
            vec![
                0;
                liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, 2,)
                    .expect("Position width")
            ];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 0,
                market_account: claims_market.to_bytes(),
                owner: position_owner.pubkey().to_bytes(),
                basis_id: [0x22; 32],
            },
            &[0, 0],
            &mut expected_position,
        )
        .expect("Position bytes");

        let instructions = vec![
            solana_system_interface::instruction::transfer(
                &position_owner.pubkey(),
                &position,
                100,
            ),
            solana_system_interface::instruction::transfer(
                &position_owner.pubkey(),
                &admission,
                200,
            ),
            Instruction {
                program_id: claims_program,
                accounts: vec![
                    AccountMeta::new_readonly(claims_market, false),
                    AccountMeta::new_readonly(logical_market, false),
                    AccountMeta::new_readonly(rent_credit, false),
                    AccountMeta::new_readonly(trading_replay, false),
                ],
                data: request_bytes.to_vec(),
            },
        ];
        let recent_blockhash = Hash::new_unique();
        let message = VersionedMessage::Legacy(solana_sdk::message::Message::new_with_blockhash(
            &instructions,
            Some(&fee_payer.pubkey()),
            &recent_blockhash,
        ));
        let transaction = VersionedTransaction::try_new(
            message,
            &[&fee_payer as &dyn Signer, position_owner as &dyn Signer],
        )
        .expect("signed admission fixture");
        let message_bytes = transaction.message.serialize();
        let wire = bincode::serialize(&transaction).expect("admission packet");
        let mut prestate = BTreeMap::new();
        for (label, key, lamports) in [
            ("position_owner", position_owner.pubkey(), 5_000),
            ("fee_payer", fee_payer.pubkey(), 3_000),
            ("claims_position", position, 0),
            ("claims_admission", admission, 0),
        ] {
            prestate.insert(label.into(), account_state_fixture(key, lamports));
        }
        for (label, key) in [
            ("claims_market", claims_market),
            ("core_market", logical_market),
            ("lifecycle_rent_credit", rent_credit),
            ("founding_trading_custody_replay", trading_replay),
        ] {
            prestate.insert(label.into(), account_state_fixture(key, 11));
        }
        for key in transaction.message.static_account_keys() {
            if prestate
                .values()
                .any(|state| state.address == key.to_string())
            {
                continue;
            }
            prestate.insert(
                format!("message_account_{key}"),
                exact_account_state_fixture(*key, 1, Pubkey::new_unique(), true, Vec::new()),
            );
        }
        let intent = IntentV1 {
            plan_sha256: hex(&[0x31; 32]),
            campaign_evidence_sha256: hex(&[0x32; 32]),
            observation_slot: 91,
            observation_unix_timestamp: 1_700_000_091,
            minimum_finalized_slot: 90,
            position_owner: position_owner.pubkey().to_string(),
            fee_payer: fee_payer.pubkey().to_string(),
            claims_market: claims_market.to_string(),
            position: position.to_string(),
            admission: admission.to_string(),
            founding_trading_custody_replay: prestate["founding_trading_custody_replay"]
                .address
                .clone(),
            position_rent_principal_lamports: 100,
            admission_rent_principal_lamports: 200,
            position_top_up_lamports: 100,
            admission_top_up_lamports: 200,
            transaction_fee_lamports: 5,
            total_owner_debit_lamports: 300,
            total_fee_payer_debit_lamports: 5,
            recent_blockhash: recent_blockhash.to_string(),
            last_valid_block_height: 1_000,
            wire_bytes: wire.len(),
            message_base64: BASE64.encode(&message_bytes),
            message_sha256: sha256_hex(&message_bytes),
            claims_request_sha256: hex(&request_digest),
            expected_receipt_producer: claims_program.to_string(),
            expected_receipt_base64: BASE64.encode(expected_receipt),
            expected_position_base64: BASE64.encode(expected_position),
            expected_admission_base64: BASE64.encode(expected_admission),
            instructions: instructions.iter().map(instruction_evidence).collect(),
            prestate,
        };
        let mut report = ReportV1 {
            schema: REPORT_SCHEMA_V1.into(),
            cluster: "devnet".into(),
            rpc_url: "https://api.devnet.solana.com".into(),
            authorized_mutation: true,
            phase: PhaseV1::Submitted,
            intent_sha256: sha256_hex(&serde_json::to_vec(&intent).expect("serialize intent")),
            envelope_sha256: String::new(),
            intent,
            signed_packet_base64: Some(BASE64.encode(&wire)),
            signed_packet_sha256: Some(sha256_hex(&wire)),
            expected_signature: Some(transaction.signatures[0].to_string()),
            finalized: None,
            collateral: None,
        };
        report.envelope_sha256 = admission_envelope_digest(&report).expect("admission envelope");
        (report, transaction)
    }

    fn admission_balance_meta(report: &ReportV1, transaction: &VersionedTransaction) -> Value {
        let mut pre = Vec::new();
        let mut post = Vec::new();
        for key in transaction.message.static_account_keys() {
            let before = report
                .intent
                .prestate
                .values()
                .find(|state| state.address == key.to_string())
                .expect("message account prestate")
                .lamports;
            let mut after = before;
            if key.to_string() == report.intent.position_owner {
                after -= report.intent.position_top_up_lamports
                    + report.intent.admission_top_up_lamports;
            }
            if key.to_string() == report.intent.fee_payer {
                after -= report.intent.transaction_fee_lamports;
            }
            if key.to_string() == report.intent.position {
                after += report.intent.position_top_up_lamports;
            }
            if key.to_string() == report.intent.admission {
                after += report.intent.admission_top_up_lamports;
            }
            pre.push(before);
            post.push(after);
        }
        json!({"preBalances": pre, "postBalances": post})
    }

    /// The fixture's report, walked back to the unsigned shape `build_report`
    /// leaves behind: a plan, a message, and no signature over it.
    fn planned_unsigned_fixture() -> (ReportV1, Keypair) {
        let owner = Keypair::new();
        let (mut report, _) = admission_exactness_fixture_for(&owner);
        report.phase = PhaseV1::Planned;
        report.signed_packet_base64 = None;
        report.signed_packet_sha256 = None;
        report.expected_signature = None;
        report.intent_sha256 =
            sha256_hex(&serde_json::to_vec(&report.intent).expect("serialize intent"));
        report.envelope_sha256 = admission_envelope_digest(&report).expect("envelope");
        (report, owner)
    }

    #[test]
    fn an_owner_who_pays_its_own_fee_is_refused_before_the_chain_charges_for_it() {
        // The frame requires the Position owner to sign READONLY. A fee payer
        // is writable unconditionally, so owner == fee payer is unsatisfiable,
        // and cohort-11 paid 12,233 CU and a fee to learn it as `Content`.
        let owner = Keypair::new();
        let other = Pubkey::new_unique();
        let declared = vec![Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![
                // Exactly the frame's shape for the owner: signs, readonly.
                AccountMeta::new_readonly(owner.pubkey(), true),
                AccountMeta::new(other, false),
            ],
            data: vec![7],
        }];

        // The pure guard, which runs on the FIRST plan and therefore before the
        // prefund spends anything. It needs no compiled message at all.
        let error = require_fee_payer_never_declared_readonly_v1(&declared, owner.pubkey())
            .expect_err("an owner paying its own fee cannot sign readonly");
        let text = format!("{error:?}");
        assert!(text.contains(&owner.pubkey().to_string()), "{text}");
        assert!(text.contains("FEE PAYER"), "{text}");
        assert!(text.contains("--fee-payer"), "{text}");
        let payer = Pubkey::new_unique();
        require_fee_payer_never_declared_readonly_v1(&declared, payer)
            .expect("a fee payer no meta names readonly is admissible");

        // And the compiled comparison itself: the same plan compiles to a
        // writable owner under its own fee payer, and to exactly the declared
        // privileges under a distinct one -- which is the repair that made
        // cohort-11's two strangers admit.
        let refused = VersionedMessage::Legacy(Message::new_with_blockhash(
            &declared,
            Some(&owner.pubkey()),
            &Hash::new_unique(),
        ));
        let error = authenticate_compiled_privileges_v1(&refused, &declared)
            .expect_err("the compiled message promoted the owner to writable");
        let text = format!("{error:?}");
        assert!(text.contains("writable=true"), "{text}");
        assert!(
            text.contains("declared signer=true writable=false"),
            "{text}"
        );

        let accepted = VersionedMessage::Legacy(Message::new_with_blockhash(
            &declared,
            Some(&payer),
            &Hash::new_unique(),
        ));
        authenticate_compiled_privileges_v1(&accepted, &declared)
            .expect("a distinct fee payer leaves the owner readonly");
    }

    #[test]
    fn a_slow_prefund_does_not_get_to_sign_the_blockhash_its_plan_was_compiled_with() {
        // `build_report` compiles the message at the blockhash it can see, and
        // then the driver spends a finalized re-observation, a fresh fee quote,
        // a poststate probe and a prestate re-read before it may sign. On
        // devnet at six blocks a second the hundred-and-fifty-block life of
        // that blockhash is twenty-five seconds, which is shorter than the
        // pass -- so the fixture stands in for a prefund slow enough to consume
        // the whole life, and the fact under test is that what gets SIGNED
        // carries the blockhash bound last, not the one the plan was drawn at.
        let (mut report, owner) = planned_unsigned_fixture();
        let plan_time = report.intent.recent_blockhash.clone();
        let plan_time_height = report.intent.last_valid_block_height;
        let durable_wire_bytes = report.intent.wire_bytes;

        let signing_time = Hash::new_unique();
        require_rebindable_unsigned_admission_v1(&report).expect("an unsigned plan may rebind");
        rebind_intent_blockhash_v1(&mut report.intent, signing_time, plan_time_height + 4_242)
            .expect("rebind the unsigned plan");

        assert_ne!(plan_time, signing_time.to_string());
        assert_eq!(report.intent.recent_blockhash, signing_time.to_string());
        assert_eq!(
            report.intent.last_valid_block_height,
            plan_time_height + 4_242
        );

        // The bytes `sign_and_submit` will sign, signed the way it signs them.
        let message_bytes = BASE64
            .decode(&report.intent.message_base64)
            .expect("rebound message base64");
        assert_eq!(sha256_hex(&message_bytes), report.intent.message_sha256);
        let message: VersionedMessage =
            bincode::deserialize(&message_bytes).expect("rebound message");
        assert_eq!(message.recent_blockhash(), &signing_time);
        let fee_payer = Pubkey::from_str(&report.intent.fee_payer).expect("fee payer");
        assert_eq!(message.static_account_keys().first(), Some(&fee_payer));

        // And the durable wire width `sign_and_submit` refuses a mismatch
        // against is unchanged, because a blockhash is a fixed thirty-two
        // bytes wherever it sits.
        let signed = VersionedTransaction {
            signatures: vec![Signature::default(), owner.sign_message(&message_bytes)],
            message,
        };
        let wire = bincode::serialize(&signed).expect("rebound packet");
        assert_eq!(wire.len(), durable_wire_bytes);
    }

    #[test]
    fn a_signed_admission_is_never_rebound_onto_a_fresh_blockhash() {
        // The wall the rebind stands behind. Once a signature over these bytes
        // exists, replacing them is a second signature over the same intent --
        // which is the replay this driver refuses; an expired signed packet is
        // archived instead.
        let (planned, _) = planned_unsigned_fixture();
        require_rebindable_unsigned_admission_v1(&planned).expect("unsigned Planned rebinds");

        let (submitted, _) = admission_exactness_fixture();
        assert!(submitted.expected_signature.is_some());
        assert!(require_rebindable_unsigned_admission_v1(&submitted).is_err());

        for phase in [
            PhaseV1::SignedNotSubmitted,
            PhaseV1::Dispatching,
            PhaseV1::Submitted,
            PhaseV1::Finalized,
        ] {
            let mut report = planned.clone();
            report.phase = phase;
            assert!(
                require_rebindable_unsigned_admission_v1(&report).is_err(),
                "phase {phase:?} must not rebind"
            );
        }

        // A Planned report is still refused the moment any signature evidence
        // is attached to it, whatever its phase claims.
        let (signed_shape, _) = admission_exactness_fixture();
        for mutate in [
            (|report: &mut ReportV1, source: &ReportV1| {
                report.signed_packet_base64 = source.signed_packet_base64.clone()
            }) as fn(&mut ReportV1, &ReportV1),
            |report, source| report.signed_packet_sha256 = source.signed_packet_sha256.clone(),
            |report, source| report.expected_signature = source.expected_signature.clone(),
        ] {
            let mut report = planned.clone();
            mutate(&mut report, &signed_shape);
            assert!(require_rebindable_unsigned_admission_v1(&report).is_err());
        }
    }

    #[test]
    fn a_rebind_over_a_message_that_is_not_its_durable_digest_refuses() {
        let (mut report, _) = planned_unsigned_fixture();
        report.intent.message_sha256 = hex(&[0x77; 32]);
        assert!(
            rebind_intent_blockhash_v1(&mut report.intent, Hash::new_unique(), 1).is_err(),
            "a message whose digest is not the durable one is not the planned message"
        );
    }

    #[test]
    fn fresh_semantic_reconstruction_refuses_a_self_rehashed_rewritten_intent() {
        let (mut report, _) = admission_exactness_fixture();
        let canonical = report.intent.clone();
        require_exact_reconstructed_admission_intent_v1(&report.intent, &canonical)
            .expect("exact fresh reconstruction");

        report.intent.total_owner_debit_lamports += 1;
        report.intent_sha256 =
            sha256_hex(&serde_json::to_vec(&report.intent).expect("rewritten intent"));
        authenticate_intent_digest(&report).expect("attacker supplied a matching self-hash");
        assert!(
            require_exact_reconstructed_admission_intent_v1(&report.intent, &canonical).is_err()
        );
    }

    #[test]
    fn admission_packet_rejects_bad_secondary_signature_and_nonexact_history_wire() {
        let (mut report, mut transaction) = admission_exactness_fixture();
        authenticate_admission_packet(&report).expect("canonical signed packet");
        assert!(transaction.signatures.len() >= 2);
        transaction.signatures[1] = Signature::from([0x71; 64]);
        let hostile_wire = bincode::serialize(&transaction).expect("hostile packet");
        report.signed_packet_base64 = Some(BASE64.encode(&hostile_wire));
        report.signed_packet_sha256 = Some(sha256_hex(&hostile_wire));
        assert!(authenticate_admission_packet(&report).is_err());

        let (report, transaction) = admission_exactness_fixture();
        let canonical_wire = bincode::serialize(&transaction).expect("canonical wire");
        require_exact_history_wire_v1(
            report
                .signed_packet_base64
                .as_deref()
                .expect("durable packet"),
            &canonical_wire,
        )
        .expect("exact history wire");
        let mut substituted = canonical_wire.clone();
        *substituted.last_mut().expect("packet byte") ^= 1;
        assert!(
            require_exact_history_wire_v1(
                report
                    .signed_packet_base64
                    .as_deref()
                    .expect("durable packet"),
                &substituted,
            )
            .is_err()
        );
        decode_exact_base64_tuple_v1(
            &json!([BASE64.encode(&canonical_wire), "base64"]),
            "canonical tuple",
        )
        .expect("exact base64 tuple");
        assert!(
            decode_exact_base64_tuple_v1(
                &json!([BASE64.encode(&canonical_wire), "base58"]),
                "wrong encoding",
            )
            .is_err()
        );
        assert!(
            decode_exact_base64_tuple_v1(
                &json!([BASE64.encode(&canonical_wire), "base64", "surplus"]),
                "surplus tuple",
            )
            .is_err()
        );
    }

    #[test]
    fn admission_finalized_balance_vector_binds_ordered_keys_fee_and_rent() {
        let (report, transaction) = admission_exactness_fixture();
        let meta = admission_balance_meta(&report, &transaction);
        authenticate_admission_balance_vector(&meta, &transaction.message, &report.intent)
            .expect("exact admission balance vector");

        let mut wrong_pre = meta.clone();
        wrong_pre["preBalances"][0] =
            json!(wrong_pre["preBalances"][0].as_u64().expect("pre-balance") + 1);
        assert!(
            authenticate_admission_balance_vector(
                &wrong_pre,
                &transaction.message,
                &report.intent,
            )
            .is_err()
        );

        let mut wrong_post = meta;
        let unchanged = transaction
            .message
            .static_account_keys()
            .iter()
            .position(|key| {
                ![
                    &report.intent.position_owner,
                    &report.intent.fee_payer,
                    &report.intent.position,
                    &report.intent.admission,
                ]
                .contains(&&key.to_string())
            })
            .expect("unchanged message key");
        wrong_post["postBalances"][unchanged] = json!(
            wrong_post["postBalances"][unchanged]
                .as_u64()
                .expect("post-balance")
                + 1
        );
        assert!(
            authenticate_admission_balance_vector(
                &wrong_post,
                &transaction.message,
                &report.intent,
            )
            .is_err()
        );
    }

    #[test]
    fn persisted_admission_poststate_survives_later_legitimate_position_mutation() {
        let (report, transaction) = admission_exactness_fixture();
        let meta = admission_balance_meta(&report, &transaction);
        let finalized_balances =
            authenticate_admission_balance_vector(&meta, &transaction.message, &report.intent)
                .expect("immutable transaction balance history");
        let completed =
            project_admission_poststate_from_history_v1(&report.intent, &finalized_balances)
                .expect("history-derived transaction-time poststate");
        authenticate_admission_poststate_map(&report.intent, &completed)
            .expect("persisted transaction-time poststate");

        let mut later_live = completed.clone();
        let position_key = pubkey(&report.intent.position).expect("Position");
        let position_state = &completed["claims_position"];
        let mut mutated_position = BASE64
            .decode(&report.intent.expected_position_base64)
            .expect("Position state");
        let last = mutated_position.last_mut().expect("Position balance byte");
        *last = last.wrapping_add(1);
        later_live.insert(
            "claims_position".into(),
            exact_account_state_fixture(
                position_key,
                position_state.lamports,
                pubkey(&position_state.owner).expect("Claims"),
                false,
                mutated_position,
            ),
        );
        assert!(authenticate_admission_poststate_map(&report.intent, &later_live).is_err());
        authenticate_admission_poststate_map(&report.intent, &completed)
            .expect("persisted evidence does not rot when the live Position changes");
    }

    #[test]
    fn signer_alias_refusal_precedes_any_keypair_read() {
        let shared = Pubkey::new_unique();
        assert!(
            enforce_shared_key_path(
                shared,
                Path::new("/private/tmp/first-key.json"),
                shared,
                Path::new("/private/tmp/second-key.json"),
            )
            .is_err()
        );
        enforce_shared_key_path(
            shared,
            Path::new("/private/tmp/shared-key.json"),
            shared,
            Path::new("/private/tmp/shared-key.json"),
        )
        .expect("one explicit shared key path");
    }

    struct CustodyActivationFixtureV1 {
        programdata: Pubkey,
        program_bytes: Vec<u8>,
        programdata_bytes: Vec<u8>,
        cache: Pubkey,
        cache_bytes: Vec<u8>,
        artifact_release: String,
    }

    fn custody_activation_fixture_v1(
        registry: Pubkey,
        program: Pubkey,
        release_set_id: [u8; 32],
    ) -> CustodyActivationFixtureV1 {
        let programdata =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let slot = 71;
        let authority = [0x61; 32];
        let elf = vec![0x52; 128];
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program.to_bytes()).expect("Custody program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("Loader"),
            programdata.to_bytes(),
            ContentId::new([0x53; 32]).expect("semantic release"),
            hash(&elf).to_bytes(),
            slot,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority),
        )
        .expect("Custody artifact release");
        let artifact = ArtifactReleaseIdV1::new([0x54; 32]).expect("artifact release ID");
        let binding = ExecutionRoleBindingV1::new(release.program(), artifact);
        let release_set = ExecutionReleaseSetV1::new(binding, binding, binding, binding, binding)
            .expect("release set");
        let observation = DeploymentObservationV1::new(
            program.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            programdata.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            programdata.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            slot,
            release.elf_digest(),
            Some(authority),
        )
        .expect("deployment observation");
        let input = ArtifactActivationInputV1::new(artifact, release, observation);
        let activated = activate_execution_release_set_v1(
            ContentId::new(release_set_id).expect("release set ID"),
            &release_set,
            &ExecutionReleaseActivationInputsV1::new(input, input, input, input, input),
        )
        .expect("activation cache");
        let cache = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_slice()],
            &registry,
        )
        .0;
        let mut program_bytes = vec![0_u8; 36];
        program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(programdata.as_ref());
        let mut programdata_bytes = vec![0_u8; 45 + elf.len()];
        programdata_bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
        programdata_bytes[4..12].copy_from_slice(&slot.to_le_bytes());
        programdata_bytes[12] = 1;
        programdata_bytes[13..45].copy_from_slice(&authority);
        programdata_bytes[45..].copy_from_slice(&elf);
        CustodyActivationFixtureV1 {
            programdata,
            program_bytes,
            programdata_bytes,
            cache,
            cache_bytes: activated.to_bytes().to_vec(),
            artifact_release: hex(&artifact.to_bytes()),
        }
    }

    fn collateral_intent_fixture() -> (CollateralIntentV1, Vec<u8>) {
        let participant = Keypair::new();
        let (intent, delegated, _) = collateral_intent_and_packet_fixture(&participant, [1; 32]);
        (intent, delegated)
    }

    fn collateral_intent_and_packet_fixture(
        participant_keypair: &Keypair,
        admission_intent_digest: [u8; 32],
    ) -> (CollateralIntentV1, Vec<u8>, VersionedTransaction) {
        let mint = Pubkey::new_unique();
        let participant = participant_keypair.pubkey();
        let market = Pubkey::new_unique();
        let release_set = [4; 32];
        let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        let registry_program = Pubkey::new_unique();
        let core_program = Pubkey::new_unique();
        let custody_program = Pubkey::new_unique();
        let activation =
            custody_activation_fixture_v1(registry_program, custody_program, release_set);
        let custody_authority = Pubkey::find_program_address(
            &CustodyAuthoritySeedsV1::new(market.to_bytes(), release_set).as_slices(),
            &custody_program,
        )
        .0;
        let participant_seed = participant_collateral_seed_v1(market, participant, release_set);
        let participant_token =
            Pubkey::create_with_seed(&participant, &participant_seed, &token_program)
                .expect("participant token derivation");
        let source = Pubkey::new_unique();
        let source_owner_keypair = Keypair::new();
        let source_owner = source_owner_keypair.pubkey();
        let fee_payer_keypair = Keypair::new();
        let fee_payer = fee_payer_keypair.pubkey();
        let quantity = 0x0807_0605_0403_0201;
        let base = TokenAccount::initialized_base_bytes(mint.to_bytes(), participant.to_bytes())
            .expect("initialized base");
        let delegated = TokenAccount::project_delegated_source_poststate(
            &base,
            quantity,
            COption::Some(custody_authority.to_bytes()),
            quantity,
        )
        .expect("delegated poststate");
        let source_base =
            TokenAccount::initialized_base_bytes(mint.to_bytes(), source_owner.to_bytes())
                .expect("source base");
        let source_pre = TokenAccount::project_amount_poststate(
            &source_base,
            quantity.checked_add(100).expect("source balance"),
        )
        .expect("source prestate");
        let source_post =
            TokenAccount::project_amount_poststate(&source_pre, 100).expect("source poststate");
        let mut mint_pre = [0_u8; MINT_BYTES];
        mint_pre[36..44].copy_from_slice(&quantity.checked_add(100).expect("supply").to_le_bytes());
        mint_pre[44] = 9;
        mint_pre[45] = 1;
        Mint::parse(&mint_pre).expect("canonical test Mint");
        let adapter_release: [u8; 32] = Sha256::digest(
            CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer().to_bytes(),
        )
        .into();
        let realm_bytes = RealmV1::new(RealmV1Input {
            token_program: TOKEN_2022_PROGRAM_ID,
            collateral_mint: mint.to_bytes(),
            collateral_adapter_release_id: adapter_release,
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("Realm")
        .to_bytes();
        let realm_digest: [u8; 32] = Sha256::digest(realm_bytes).into();
        let realm_record = Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                REALM_SCHEMA_RELEASE_ID_V1.as_slice(),
                realm_digest.as_slice(),
            ],
            &registry_program,
        )
        .0;
        let id = |byte| Identity::new([byte; 32]).expect("fixture identity");
        let market_bytes = CoreState {
            phase: CorePhase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity: MarketIdentity {
                market_id: Identity::new(market.to_bytes()).expect("market identity"),
                realm_id: Identity::new(realm_digest).expect("Realm identity"),
                product_record: id(0x31),
                product_id: id(0x32),
                resolution_policy: id(0x33),
                capability_manifest: id(0x34),
                selected_release_set: Identity::new(release_set).expect("release set identity"),
                registry_program: Identity::new(registry_program.to_bytes())
                    .expect("Registry identity"),
                generation: 1,
            },
            outstanding_capabilities: 0,
            principal_cap_sets: u64::MAX,
            rent_beneficiary: id(0x35),
            terminal_receipt: None,
            bumps: StateBumpsV1::UNRECORDED,
        }
        .encode()
        .expect("Open Market");
        let mut prestate = BTreeMap::new();
        prestate.insert(
            "fee_payer".into(),
            account_state_fixture(fee_payer, 1_000_000),
        );
        let source_rpc = RpcAccount {
            lamports: 2_039_280,
            owner: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
            executable: false,
            rent_epoch: 0,
            data: source_pre.to_vec(),
        };
        prestate.insert(
            "source_account".into(),
            account_state(source, Some(&source_rpc)),
        );
        prestate.insert(
            "participant_token_account".into(),
            account_state(participant_token, None),
        );
        let mint_rpc = RpcAccount {
            lamports: 1_461_600,
            owner: token_program,
            executable: false,
            rent_epoch: 0,
            data: mint_pre.to_vec(),
        };
        prestate.insert("mint".into(), account_state(mint, Some(&mint_rpc)));
        let market_rpc = RpcAccount {
            lamports: 3_000_000,
            owner: core_program,
            executable: false,
            rent_epoch: 0,
            data: market_bytes.to_vec(),
        };
        prestate.insert("market".into(), account_state(market, Some(&market_rpc)));
        let realm_rpc = RpcAccount {
            lamports: 2_000_000,
            owner: registry_program,
            executable: false,
            rent_epoch: 0,
            data: realm_bytes.to_vec(),
        };
        prestate.insert(
            "realm_record".into(),
            account_state(realm_record, Some(&realm_rpc)),
        );
        let registry_programdata =
            Pubkey::find_program_address(&[registry_program.as_ref()], &bpf_loader_upgradeable::ID)
                .0;
        let mut registry_program_bytes = vec![0_u8; 36];
        registry_program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        registry_program_bytes[4..].copy_from_slice(registry_programdata.as_ref());
        let registry_rpc = RpcAccount {
            lamports: 1,
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
            data: registry_program_bytes,
        };
        prestate.insert(
            "registry_program".into(),
            account_state(registry_program, Some(&registry_rpc)),
        );
        let activation_rpc = RpcAccount {
            lamports: 4_000_000,
            owner: registry_program,
            executable: false,
            rent_epoch: 0,
            data: activation.cache_bytes.clone(),
        };
        prestate.insert(
            "activation_cache".into(),
            account_state(activation.cache, Some(&activation_rpc)),
        );
        let custody_program_rpc = RpcAccount {
            lamports: 1,
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
            data: activation.program_bytes.clone(),
        };
        prestate.insert(
            "custody_program".into(),
            account_state(custody_program, Some(&custody_program_rpc)),
        );
        let custody_programdata_rpc = RpcAccount {
            lamports: 8_000_000,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
            data: activation.programdata_bytes.clone(),
        };
        prestate.insert(
            "custody_programdata".into(),
            account_state(activation.programdata, Some(&custody_programdata_rpc)),
        );
        let mut intent = CollateralIntentV1 {
            admission_intent_sha256: hex(&admission_intent_digest),
            observation_slot: 1,
            observation_unix_timestamp: 1,
            minimum_finalized_slot: 1,
            market: market.to_string(),
            registry_program: registry_program.to_string(),
            realm_record: realm_record.to_string(),
            realm_data_sha256: sha256_hex(&realm_bytes),
            collateral_adapter_release: hex(&adapter_release),
            release_set: hex(&release_set),
            activation_cache: activation.cache.to_string(),
            custody_program: custody_program.to_string(),
            custody_programdata: activation.programdata.to_string(),
            custody_artifact_release: activation.artifact_release,
            custody_authority: custody_authority.to_string(),
            token_program: token_program.to_string(),
            mint: mint.to_string(),
            mint_decimals: 9,
            participant: participant.to_string(),
            participant_token_seed: participant_seed,
            participant_token_account: participant_token.to_string(),
            source_owner: source_owner.to_string(),
            source_account: source.to_string(),
            quantity_atoms: quantity,
            creates_participant_account: true,
            participant_account_rent_lamports: 5_000,
            transaction_fee_lamports: 2_000,
            total_fee_payer_debit_lamports: 7_000,
            fee_payer: fee_payer.to_string(),
            recent_blockhash: Hash::new_unique().to_string(),
            last_valid_block_height: 2,
            wire_bytes: 0,
            message_base64: String::new(),
            message_sha256: hex(&[5; 32]),
            expected_return_data: None,
            market_pre_base64: BASE64.encode(market_bytes),
            realm_pre_base64: BASE64.encode(realm_bytes),
            registry_program_pre_base64: BASE64.encode(&registry_rpc.data),
            activation_cache_pre_base64: BASE64.encode(&activation_rpc.data),
            custody_program_pre_base64: BASE64.encode(&custody_program_rpc.data),
            custody_programdata_pre_base64: BASE64.encode(&custody_programdata_rpc.data),
            mint_pre_base64: BASE64.encode(mint_pre),
            source_pre_base64: BASE64.encode(source_pre),
            participant_pre_base64: None,
            participant_transfer_pre_base64: BASE64.encode(base),
            expected_source_base64: BASE64.encode(source_post),
            expected_participant_token_base64: BASE64.encode(delegated),
            instructions: Vec::new(),
            prestate,
        };
        let instructions = canonical_collateral_instructions(&intent).expect("instructions");
        let compiled = compile_v0_message_with_optional_tables(
            fee_payer,
            &instructions,
            intent.recent_blockhash.parse().expect("blockhash"),
            Observation {
                finality: Finality::Finalized,
                slot: intent.observation_slot,
                unix_timestamp: intent.observation_unix_timestamp,
            },
            &[],
        )
        .expect("compile message");
        let transaction = VersionedTransaction::try_new(
            compiled.message.clone(),
            &[
                &fee_payer_keypair as &dyn Signer,
                participant_keypair as &dyn Signer,
                &source_owner_keypair as &dyn Signer,
            ],
        )
        .expect("signed collateral fixture");
        let wire = bincode::serialize(&transaction).expect("collateral packet");
        let message = transaction.message.serialize();
        intent.wire_bytes = wire.len();
        intent.message_base64 = BASE64.encode(&message);
        intent.message_sha256 = sha256_hex(&message);
        intent.instructions = instructions.iter().map(instruction_evidence).collect();
        (intent, delegated.to_vec(), transaction)
    }

    #[test]
    fn direct_projection_exposes_only_joined_finalized_participant_facts() {
        let (mut report, _) = admission_exactness_fixture();
        let (collateral_intent, _) = collateral_intent_fixture();
        report.schema = LOCAL_REPORT_SCHEMA_V1.into();
        report.cluster = ExpectedClusterV1::OwnedLoopback.evidence_label().into();
        report.intent.position_owner = collateral_intent.participant.clone();
        report.phase = PhaseV1::Finalized;
        report.finalized = Some(FinalizedEvidenceV1 {
            signature: Signature::default().to_string(),
            slot: 41,
            fee_lamports: 1,
            compute_units_consumed: Some(2),
            return_data_producer: Pubkey::new_unique().to_string(),
            return_data_sha256: hex(&[0x41; 32]),
            poststate: BTreeMap::new(),
        });
        report.collateral = Some(CollateralReportV1 {
            phase: CollateralPhaseV1::Finalized,
            intent_sha256: hex(&[0x42; 32]),
            envelope_sha256: hex(&[0x43; 32]),
            intent: collateral_intent.clone(),
            signed_packet_base64: Some("packet".into()),
            signed_packet_sha256: Some(hex(&[0x44; 32])),
            expected_signature: Some(Signature::default().to_string()),
            finalized: Some(CollateralFinalizedEvidenceV1 {
                signature: Signature::default().to_string(),
                slot: 42,
                fee_lamports: 3,
                compute_units_consumed: Some(4),
                return_data: None,
                poststate: BTreeMap::new(),
            }),
        });

        let projected = project_finalized_direct_participant_evidence_v1(&report)
            .expect("joined finalized Direct participant");
        assert_eq!(
            projected.owner,
            pubkey(&collateral_intent.participant).expect("participant")
        );
        assert_eq!(
            projected.collateral_account,
            pubkey(&collateral_intent.participant_token_account).expect("collateral account")
        );
        assert_eq!(
            projected.collateral_quantity_atoms,
            collateral_intent.quantity_atoms
        );
        assert_eq!(projected.admission_slot, 41);
        assert_eq!(projected.collateral_slot, 42);

        let mut foreign_owner = report.clone();
        foreign_owner.intent.position_owner = Pubkey::new_unique().to_string();
        assert!(project_finalized_direct_participant_evidence_v1(&foreign_owner).is_err());

        let mut zero_quantity = report.clone();
        zero_quantity
            .collateral
            .as_mut()
            .expect("collateral")
            .intent
            .quantity_atoms = 0;
        assert!(project_finalized_direct_participant_evidence_v1(&zero_quantity).is_err());

        let mut missing_history = report;
        missing_history
            .collateral
            .as_mut()
            .expect("collateral")
            .finalized = None;
        assert!(project_finalized_direct_participant_evidence_v1(&missing_history).is_err());
    }

    #[test]
    fn offline_direct_projection_authenticates_exact_devnet_packets_and_cluster() {
        let participant = Keypair::new();
        let (mut report, _) = admission_exactness_fixture_for(&participant);
        let admission_digest: [u8; 32] =
            Sha256::digest(serde_json::to_vec(&report.intent).expect("serialize admission intent"))
                .into();
        let (collateral_intent, _, collateral_transaction) =
            collateral_intent_and_packet_fixture(&participant, admission_digest);
        let collateral_wire =
            bincode::serialize(&collateral_transaction).expect("collateral packet");

        report.phase = PhaseV1::Finalized;
        let admission_signature = report
            .expected_signature
            .clone()
            .expect("admission signature");
        report.finalized = Some(FinalizedEvidenceV1 {
            signature: admission_signature,
            slot: 41,
            fee_lamports: 1,
            compute_units_consumed: Some(2),
            return_data_producer: Pubkey::new_unique().to_string(),
            return_data_sha256: hex(&[0x41; 32]),
            poststate: BTreeMap::new(),
        });
        let collateral_signature = collateral_transaction.signatures[0].to_string();
        report.collateral = Some(CollateralReportV1 {
            phase: CollateralPhaseV1::Finalized,
            intent_sha256: sha256_hex(
                &serde_json::to_vec(&collateral_intent).expect("serialize collateral intent"),
            ),
            envelope_sha256: String::new(),
            intent: collateral_intent.clone(),
            signed_packet_base64: Some(BASE64.encode(&collateral_wire)),
            signed_packet_sha256: Some(sha256_hex(&collateral_wire)),
            expected_signature: Some(collateral_signature.clone()),
            finalized: Some(CollateralFinalizedEvidenceV1 {
                signature: collateral_signature,
                slot: 42,
                fee_lamports: 3,
                compute_units_consumed: Some(4),
                return_data: None,
                poststate: BTreeMap::new(),
            }),
        });
        refresh_report_envelope_digests(&mut report).expect("phase envelope digests");

        let bytes = serde_json::to_vec(&report).expect("participant report");
        let projected = parse_finalized_direct_participant_evidence_offline_v1(
            &bytes,
            ExpectedClusterV1::Devnet,
        )
        .expect("authenticated key-free devnet projection");
        assert_eq!(projected.owner, participant.pubkey());
        assert_eq!(
            projected.market,
            pubkey(&collateral_intent.market).expect("Market")
        );
        assert_eq!(projected.admission_slot, 41);
        assert_eq!(projected.collateral_slot, 42);

        assert!(
            parse_finalized_direct_participant_evidence_offline_v1(
                &bytes,
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_err()
        );

        let mut foreign = report.clone();
        foreign.intent.position_owner = Pubkey::new_unique().to_string();
        foreign.intent_sha256 = sha256_hex(
            &serde_json::to_vec(&foreign.intent).expect("serialize hostile admission intent"),
        );
        refresh_report_envelope_digests(&mut foreign).expect("hostile envelope digests");
        assert!(
            parse_finalized_direct_participant_evidence_offline_v1(
                &serde_json::to_vec(&foreign).expect("hostile participant report"),
                ExpectedClusterV1::Devnet,
            )
            .is_err()
        );

        let mut unknown = serde_json::to_value(&report).expect("participant value");
        unknown
            .as_object_mut()
            .expect("participant object")
            .insert("secretKeyPath".into(), Value::String("forbidden".into()));
        assert!(
            parse_finalized_direct_participant_evidence_offline_v1(
                &serde_json::to_vec(&unknown).expect("unknown-field participant report"),
                ExpectedClusterV1::Devnet,
            )
            .is_err()
        );
    }

    #[test]
    fn participant_coordinates_select_the_open_markets_founding_credit_and_refuse_aliasing() {
        let key = |value: u8| Pubkey::new_from_array([value; 32]);
        let founding_credit = key(9);
        let found37_credit = key(10);
        let mut evidence = json!({
            "execution": {"market": {"accounts": {
                "claims_aggregate": {"address": key(1).to_string()},
                "founding_market": {"address": key(2).to_string()},
                "founding_lifecycle_rent_credit": {"address": founding_credit.to_string()},
                "lifecycle_rent_credit": {"address": found37_credit.to_string()},
                "founding_normal_custody_replay": {"address": key(3).to_string()},
                "product_record": {"address": key(4).to_string()},
                "result_domain_record": {"address": key(5).to_string()},
                "portfolio_record": {"address": key(6).to_string()},
                "linked_liability_basis_record": {"address": key(7).to_string()}
            }}}
        });
        assert_eq!(
            evidence_coordinates(&evidence)
                .expect("DCLTGMF3 coordinates")
                .rent_credit,
            founding_credit
        );

        evidence["execution"]["market"]["accounts"]["founding_lifecycle_rent_credit"]["address"] =
            Value::String(found37_credit.to_string());
        assert!(evidence_coordinates(&evidence).is_err());

        evidence["execution"]["market"]["accounts"]
            .as_object_mut()
            .expect("account object")
            .remove("founding_lifecycle_rent_credit");
        assert!(evidence_coordinates(&evidence).is_err());
    }

    #[test]
    fn participant_token_poststate_rejects_mint_owner_delegate_and_quantity_substitution() {
        let (intent, canonical) = collateral_intent_fixture();
        authenticate_collateral_token_byte_plan(&intent).expect("canonical token byte plan");
        authenticate_delegated_collateral_token(&intent, &canonical).expect("canonical allowance");
        let mint = pubkey(&intent.mint).expect("mint");
        let owner = pubkey(&intent.participant).expect("owner");
        let delegate = pubkey(&intent.custody_authority).expect("delegate");
        let hostile = |hostile_mint: Pubkey,
                       hostile_owner: Pubkey,
                       hostile_delegate: Pubkey,
                       amount: u64,
                       allowance: u64| {
            let base = TokenAccount::initialized_base_bytes(
                hostile_mint.to_bytes(),
                hostile_owner.to_bytes(),
            )
            .expect("hostile base");
            TokenAccount::project_delegated_source_poststate(
                &base,
                amount,
                COption::Some(hostile_delegate.to_bytes()),
                allowance,
            )
            .expect("hostile delegated account")
        };
        assert!(
            authenticate_delegated_collateral_token(
                &intent,
                &hostile(
                    Pubkey::new_unique(),
                    owner,
                    delegate,
                    intent.quantity_atoms,
                    intent.quantity_atoms,
                ),
            )
            .is_err()
        );
        assert!(
            authenticate_delegated_collateral_token(
                &intent,
                &hostile(
                    mint,
                    Pubkey::new_unique(),
                    delegate,
                    intent.quantity_atoms,
                    intent.quantity_atoms,
                ),
            )
            .is_err()
        );
        assert!(
            authenticate_delegated_collateral_token(
                &intent,
                &hostile(
                    mint,
                    owner,
                    Pubkey::new_unique(),
                    intent.quantity_atoms,
                    intent.quantity_atoms,
                ),
            )
            .is_err()
        );
        assert!(
            authenticate_delegated_collateral_token(
                &intent,
                &hostile(
                    mint,
                    owner,
                    delegate,
                    intent.quantity_atoms.saturating_sub(1),
                    intent.quantity_atoms,
                ),
            )
            .is_err()
        );
        assert!(
            authenticate_delegated_collateral_token(
                &intent,
                &hostile(
                    mint,
                    owner,
                    delegate,
                    intent.quantity_atoms,
                    intent.quantity_atoms.saturating_sub(1),
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn collateral_byte_plan_rejects_prestate_presence_and_raw_byte_substitution() {
        let (intent, _) = collateral_intent_fixture();
        authenticate_collateral_token_byte_plan(&intent).expect("canonical token byte plan");

        let mut participant_present = intent.clone();
        participant_present.participant_pre_base64 =
            Some(participant_present.participant_transfer_pre_base64.clone());
        assert!(authenticate_collateral_token_byte_plan(&participant_present).is_err());

        let mut wrong_seed = intent.clone();
        wrong_seed.participant_token_seed = "0123456789abcdef0123456789abcdef".into();
        assert!(authenticate_collateral_token_byte_plan(&wrong_seed).is_err());

        let mut wrong_delegate = intent.clone();
        wrong_delegate.custody_authority = Pubkey::new_unique().to_string();
        assert!(authenticate_collateral_token_byte_plan(&wrong_delegate).is_err());

        let mut source_post = intent.clone();
        let mut bytes = BASE64
            .decode(&source_post.expected_source_base64)
            .expect("source poststate");
        bytes[64] ^= 1;
        source_post.expected_source_base64 = BASE64.encode(bytes);
        assert!(authenticate_collateral_token_byte_plan(&source_post).is_err());

        let mut participant_pre = intent;
        participant_pre.participant_transfer_pre_base64 = BASE64.encode(
            TokenAccount::initialized_base_bytes(
                Pubkey::new_unique().to_bytes(),
                pubkey(&participant_pre.participant)
                    .expect("participant")
                    .to_bytes(),
            )
            .expect("substituted participant prestate"),
        );
        assert!(authenticate_collateral_token_byte_plan(&participant_pre).is_err());
    }

    #[test]
    fn collateral_meta_requires_explicit_null_return_data() {
        assert!(authenticate_collateral_return_data(Some(&Value::Null), None).is_ok());
        // The RPC omits the field when none was set; absent == null.
        assert!(authenticate_collateral_return_data(None, None).is_ok());
        assert!(authenticate_collateral_return_data(Some(&json!({})), None).is_err());
        assert!(
            authenticate_collateral_return_data(Some(&Value::Null), Some("unexpected")).is_err()
        );
    }

    #[test]
    fn collateral_observation_uses_exact_block_time_for_its_own_slot() {
        let observation = collateral_observation_v1(177, 1_700_000_177);
        assert_eq!(observation.finality, Finality::Finalized);
        assert_eq!(observation.slot, 177);
        assert_eq!(observation.unix_timestamp, 1_700_000_177);
        assert_ne!(observation.unix_timestamp, 1_700_000_091);
    }

    #[test]
    fn collateral_lamport_vector_is_exact_fee_and_rent_only() {
        let (intent, _) = collateral_intent_fixture();
        let fee_payer = pubkey(&intent.prestate["fee_payer"].address).expect("payer");
        let participant_token = pubkey(&intent.participant_token_account).expect("token account");
        let instruction = Instruction {
            program_id: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
            accounts: vec![AccountMeta::new(participant_token, false)],
            data: vec![1],
        };
        let message = VersionedMessage::Legacy(solana_sdk::message::Message::new_with_blockhash(
            &[instruction],
            Some(&fee_payer),
            &Hash::new_unique(),
        ));
        let pre = vec![1_000_000_u64; message.static_account_keys().len()];
        let mut post = pre.clone();
        for (index, key) in message.static_account_keys().iter().enumerate() {
            if *key == fee_payer {
                post[index] -= intent.total_fee_payer_debit_lamports;
            } else if *key == participant_token {
                post[index] += intent.participant_account_rent_lamports;
            }
        }
        let meta = json!({"preBalances": pre, "postBalances": post});
        authenticate_collateral_balance_vector(&meta, &message, &intent).expect("exact vector");

        let mut wrong = meta;
        let balances = wrong["postBalances"].as_array_mut().expect("balances");
        let unchanged_index = message
            .static_account_keys()
            .iter()
            .position(|key| *key != fee_payer && *key != participant_token)
            .expect("program key");
        balances[unchanged_index] = json!(
            balances[unchanged_index]
                .as_u64()
                .expect("lamports")
                .saturating_add(1)
        );
        assert!(authenticate_collateral_balance_vector(&wrong, &message, &intent).is_err());
    }

    fn replace_intent_account_bytes(intent: &mut CollateralIntentV1, label: &str, bytes: Vec<u8>) {
        let before = intent.prestate[label].clone();
        let address = pubkey(&before.address).expect("account address");
        let account = RpcAccount {
            lamports: before.lamports,
            owner: pubkey(&before.owner).expect("account owner"),
            executable: before.executable,
            rent_epoch: before.rent_epoch,
            data: bytes,
        };
        intent
            .prestate
            .insert(label.into(), account_state(address, Some(&account)));
    }

    #[test]
    fn collateral_chain_join_rejects_cache_programdata_release_and_slot_substitution() {
        let (intent, _) = collateral_intent_fixture();
        authenticate_collateral_chain_join_v1(&intent).expect("canonical chain join");

        let mut wrong_cache = intent.clone();
        let mut cache = BASE64
            .decode(&wrong_cache.activation_cache_pre_base64)
            .expect("activation cache");
        cache[0] ^= 1;
        wrong_cache.activation_cache_pre_base64 = BASE64.encode(&cache);
        replace_intent_account_bytes(&mut wrong_cache, "activation_cache", cache);
        assert!(authenticate_collateral_chain_join_v1(&wrong_cache).is_err());

        let mut superseded = intent.clone();
        let mut programdata = BASE64
            .decode(&superseded.custody_programdata_pre_base64)
            .expect("ProgramData");
        let slot = u64::from_le_bytes(programdata[4..12].try_into().expect("slot"));
        programdata[4..12].copy_from_slice(&slot.saturating_add(1).to_le_bytes());
        superseded.custody_programdata_pre_base64 = BASE64.encode(&programdata);
        replace_intent_account_bytes(&mut superseded, "custody_programdata", programdata);
        assert!(authenticate_collateral_chain_join_v1(&superseded).is_err());

        let mut wrong_link = intent.clone();
        let mut program = BASE64
            .decode(&wrong_link.custody_program_pre_base64)
            .expect("Program");
        program[4..].copy_from_slice(Pubkey::new_unique().as_ref());
        wrong_link.custody_program_pre_base64 = BASE64.encode(&program);
        replace_intent_account_bytes(&mut wrong_link, "custody_program", program);
        assert!(authenticate_collateral_chain_join_v1(&wrong_link).is_err());

        let mut wrong_registry_link = intent.clone();
        let mut registry = BASE64
            .decode(&wrong_registry_link.registry_program_pre_base64)
            .expect("Registry Program");
        registry[4..].copy_from_slice(Pubkey::new_unique().as_ref());
        wrong_registry_link.registry_program_pre_base64 = BASE64.encode(&registry);
        replace_intent_account_bytes(&mut wrong_registry_link, "registry_program", registry);
        assert!(authenticate_collateral_chain_join_v1(&wrong_registry_link).is_err());

        let mut wrong_artifact = intent.clone();
        wrong_artifact.custody_artifact_release = hex(&[0x91; 32]);
        assert!(authenticate_collateral_chain_join_v1(&wrong_artifact).is_err());

        let mut wrong_release_set = intent.clone();
        wrong_release_set.release_set = hex(&[0x92; 32]);
        assert!(authenticate_collateral_chain_join_v1(&wrong_release_set).is_err());

        let mut wrong_program = intent;
        wrong_program.custody_program = Pubkey::new_unique().to_string();
        assert!(authenticate_collateral_chain_join_v1(&wrong_program).is_err());
    }

    fn independent_coordinates_from_intent(
        intent: &CollateralIntentV1,
    ) -> IndependentCollateralCoordinatesV1 {
        IndependentCollateralCoordinatesV1 {
            market: intent.market.clone(),
            realm_record: intent.realm_record.clone(),
            mint: intent.mint.clone(),
            registry_program: intent.registry_program.clone(),
            release_set: intent.release_set.clone(),
            core_program: intent.prestate["market"].owner.clone(),
            custody_program: intent.custody_program.clone(),
            custody_programdata: intent.custody_programdata.clone(),
            custody_artifact_release: intent.custody_artifact_release.clone(),
            realm_data_sha256: intent.realm_data_sha256.clone(),
            mint_data_sha256: intent.prestate["mint"].data_sha256.clone(),
            custody_programdata_sha256: sha256_hex(
                &BASE64
                    .decode(&intent.custody_programdata_pre_base64)
                    .expect("Custody ProgramData"),
            ),
        }
    }

    #[test]
    fn independently_bound_campaign_refuses_an_internally_canonical_attacker_subgraph() {
        let (campaign, _) = collateral_intent_fixture();
        let (attacker, _) = collateral_intent_fixture();
        authenticate_collateral_chain_join_v1(&campaign).expect("campaign chain join");
        authenticate_collateral_chain_join_v1(&attacker)
            .expect("attacker chain is self-consistent");
        let expected = independent_coordinates_from_intent(&campaign);
        let campaign_programdata = BASE64
            .decode(&campaign.custody_programdata_pre_base64)
            .expect("campaign ProgramData");
        authenticate_independent_collateral_coordinates_v1(
            &campaign,
            &expected,
            &campaign.prestate["mint"],
            &campaign_programdata,
        )
        .expect("campaign coordinates");
        let attacker_programdata = BASE64
            .decode(&attacker.custody_programdata_pre_base64)
            .expect("attacker ProgramData");
        assert!(
            authenticate_independent_collateral_coordinates_v1(
                &attacker,
                &expected,
                &attacker.prestate["mint"],
                &attacker_programdata,
            )
            .is_err()
        );
    }

    #[test]
    fn collateral_message_rebuild_rejects_injection_order_privilege_and_wire_substitution() {
        let (intent, _) = collateral_intent_fixture();
        authenticate_collateral_message_plan(&intent).expect("canonical exact message");

        let mut injected = intent.clone();
        injected.instructions.push(InstructionEvidenceV1 {
            program_id: system_program::ID.to_string(),
            accounts: Vec::new(),
            data_base64: BASE64.encode([0x41]),
        });
        assert!(authenticate_collateral_message_plan(&injected).is_err());

        let mut reordered = intent.clone();
        reordered.instructions.swap(2, 3);
        assert!(authenticate_collateral_message_plan(&reordered).is_err());

        let mut wrong_program = intent.clone();
        wrong_program.instructions[2].program_id = system_program::ID.to_string();
        assert!(authenticate_collateral_message_plan(&wrong_program).is_err());

        let mut wrong_account = intent.clone();
        wrong_account.instructions[2].accounts[0].address = Pubkey::new_unique().to_string();
        assert!(authenticate_collateral_message_plan(&wrong_account).is_err());

        let mut wrong_privilege = intent.clone();
        wrong_privilege.instructions[2].accounts[0].writable = false;
        assert!(authenticate_collateral_message_plan(&wrong_privilege).is_err());

        let mut wrong_decimals = intent.clone();
        wrong_decimals.mint_decimals = wrong_decimals.mint_decimals.saturating_sub(1);
        assert!(authenticate_collateral_token_byte_plan(&wrong_decimals).is_err());
        assert!(authenticate_collateral_message_plan(&wrong_decimals).is_err());

        let mut wrong_blockhash = intent.clone();
        wrong_blockhash.recent_blockhash = Hash::new_unique().to_string();
        assert!(authenticate_collateral_message_plan(&wrong_blockhash).is_err());

        let mut wrong_wire = intent;
        let mut message = BASE64
            .decode(&wrong_wire.message_base64)
            .expect("message bytes");
        let last = message.last_mut().expect("nonempty message");
        *last ^= 1;
        wrong_wire.message_base64 = BASE64.encode(message);
        assert!(authenticate_collateral_message_plan(&wrong_wire).is_err());
    }

    #[test]
    fn collateral_packet_rejects_pasted_signature_and_nonexact_history_wire() {
        let (intent, _) = collateral_intent_fixture();
        let message_bytes = BASE64
            .decode(&intent.message_base64)
            .expect("collateral message");
        let message: VersionedMessage =
            bincode::deserialize(&message_bytes).expect("versioned message");
        let required = match &message {
            VersionedMessage::Legacy(message) => {
                usize::from(message.header.num_required_signatures)
            }
            VersionedMessage::V0(message) => usize::from(message.header.num_required_signatures),
        };
        let pasted = Keypair::new().sign_message(b"unrelated finalized transaction");
        let mut signatures = vec![Signature::default(); required];
        signatures[0] = pasted;
        let packet = VersionedTransaction {
            signatures,
            message,
        };
        let wire = bincode::serialize(&packet).expect("hostile packet");
        assert_eq!(wire.len(), intent.wire_bytes);
        let report = CollateralReportV1 {
            phase: CollateralPhaseV1::Submitted,
            intent_sha256: sha256_hex(
                &serde_json::to_vec(&intent).expect("serialize collateral intent"),
            ),
            envelope_sha256: String::new(),
            intent,
            signed_packet_base64: Some(BASE64.encode(&wire)),
            signed_packet_sha256: Some(sha256_hex(&wire)),
            expected_signature: Some(pasted.to_string()),
            finalized: None,
        };
        assert!(authenticate_collateral_packet(&report).is_err());
        require_exact_history_wire_v1(
            report
                .signed_packet_base64
                .as_deref()
                .expect("durable packet"),
            &wire,
        )
        .expect("exact history packet");
        let mut substituted = wire;
        *substituted.last_mut().expect("packet byte") ^= 1;
        assert!(
            require_exact_history_wire_v1(
                report
                    .signed_packet_base64
                    .as_deref()
                    .expect("durable packet"),
                &substituted,
            )
            .is_err()
        );
    }

    #[test]
    fn collateral_economics_bind_payer_create_mode_exact_rent_fee_and_total() {
        let (intent, _) = collateral_intent_fixture();
        authenticate_collateral_economics_against_fresh_v1(
            &intent,
            intent.participant_account_rent_lamports,
            intent.transaction_fee_lamports,
        )
        .expect("canonical fresh economics");

        let mut wrong_payer = intent.clone();
        wrong_payer.fee_payer = Pubkey::new_unique().to_string();
        assert!(authenticate_collateral_economics_v1(&wrong_payer).is_err());

        let mut overfunded_rent = intent.clone();
        overfunded_rent.participant_account_rent_lamports += 1;
        overfunded_rent.total_fee_payer_debit_lamports += 1;
        assert!(
            authenticate_collateral_economics_against_fresh_v1(
                &overfunded_rent,
                intent.participant_account_rent_lamports,
                intent.transaction_fee_lamports,
            )
            .is_err()
        );

        let mut wrong_fee = intent.clone();
        wrong_fee.transaction_fee_lamports += 1;
        wrong_fee.total_fee_payer_debit_lamports += 1;
        assert!(
            authenticate_collateral_economics_against_fresh_v1(
                &wrong_fee,
                intent.participant_account_rent_lamports,
                intent.transaction_fee_lamports,
            )
            .is_err()
        );

        let mut wrong_total = intent.clone();
        wrong_total.total_fee_payer_debit_lamports += 1;
        assert!(authenticate_collateral_economics_v1(&wrong_total).is_err());

        let mut existing_with_rent = intent;
        existing_with_rent.creates_participant_account = false;
        assert!(authenticate_collateral_economics_v1(&existing_with_rent).is_err());
    }

    #[test]
    fn persisted_collateral_poststate_survives_later_legitimate_token_mutation() {
        let (intent, _) = collateral_intent_fixture();
        let mut completed = intent.prestate.clone();
        let source_key = pubkey(&intent.source_account).expect("source");
        let source_before = intent.prestate["source_account"].clone();
        let source = RpcAccount {
            lamports: source_before.lamports,
            owner: pubkey(&intent.token_program).expect("Token-2022"),
            executable: false,
            rent_epoch: source_before.rent_epoch,
            data: BASE64
                .decode(&intent.expected_source_base64)
                .expect("source poststate"),
        };
        completed.insert(
            "source_account".into(),
            account_state(source_key, Some(&source)),
        );
        let participant_key = pubkey(&intent.participant_token_account).expect("participant token");
        let participant_before = intent.prestate["participant_token_account"].clone();
        let participant = RpcAccount {
            lamports: participant_before
                .lamports
                .checked_add(intent.participant_account_rent_lamports)
                .expect("participant rent"),
            owner: pubkey(&intent.token_program).expect("Token-2022"),
            executable: false,
            rent_epoch: participant_before.rent_epoch,
            data: BASE64
                .decode(&intent.expected_participant_token_base64)
                .expect("participant poststate"),
        };
        completed.insert(
            "participant_token_account".into(),
            account_state(participant_key, Some(&participant)),
        );
        let payer_key = pubkey(&intent.fee_payer).expect("payer");
        let payer_before = intent.prestate["fee_payer"].clone();
        let payer = RpcAccount {
            lamports: payer_before
                .lamports
                .checked_sub(intent.total_fee_payer_debit_lamports)
                .expect("payer debit"),
            owner: system_program::ID,
            executable: false,
            rent_epoch: payer_before.rent_epoch,
            data: Vec::new(),
        };
        completed.insert("fee_payer".into(), account_state(payer_key, Some(&payer)));
        authenticate_collateral_poststate_map(&intent, &completed)
            .expect("persisted transaction-time state");

        let mut wrong_address = completed.clone();
        wrong_address
            .get_mut("source_account")
            .expect("source state")
            .address = Pubkey::new_unique().to_string();
        assert!(authenticate_collateral_poststate_map(&intent, &wrong_address).is_err());

        let mut wrong_account_digest = completed.clone();
        wrong_account_digest
            .get_mut("participant_token_account")
            .expect("participant state")
            .account_sha256 = hex(&[0x81; 32]);
        assert!(authenticate_collateral_poststate_map(&intent, &wrong_account_digest).is_err());

        let mut wrong_payer_digest = completed.clone();
        wrong_payer_digest
            .get_mut("fee_payer")
            .expect("payer state")
            .account_sha256 = hex(&[0x82; 32]);
        assert!(authenticate_collateral_poststate_map(&intent, &wrong_payer_digest).is_err());

        let mut wrong_rent_epoch = completed.clone();
        let changed_source = RpcAccount {
            rent_epoch: source.rent_epoch.saturating_add(1),
            ..source.clone()
        };
        wrong_rent_epoch.insert(
            "source_account".into(),
            account_state(source_key, Some(&changed_source)),
        );
        assert!(authenticate_collateral_poststate_map(&intent, &wrong_rent_epoch).is_err());

        let mut later_live = completed.clone();
        let mut spent = participant.data;
        spent[64] ^= 1;
        let later_participant = RpcAccount {
            data: spent,
            ..participant
        };
        later_live.insert(
            "participant_token_account".into(),
            account_state(participant_key, Some(&later_participant)),
        );
        assert!(authenticate_collateral_poststate_map(&intent, &later_live).is_err());
        authenticate_collateral_poststate_map(&intent, &completed)
            .expect("immutable completion evidence does not rot with live state");
    }

    #[test]
    fn phase_envelopes_reject_cross_phase_fields_and_detect_signed_reversion() {
        assert!(
            validate_phase_fields(
                "admission",
                EnvelopePhaseV1::Planned,
                Some("packet"),
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            validate_phase_fields(
                "collateral",
                EnvelopePhaseV1::SignedNotSubmitted,
                Some("packet"),
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            validate_phase_fields(
                "collateral",
                EnvelopePhaseV1::Finalized,
                Some("packet"),
                Some("digest"),
                Some("first"),
                Some("other"),
            )
            .is_err()
        );

        let (intent, _) = collateral_intent_fixture();
        let mut collateral = CollateralReportV1 {
            phase: CollateralPhaseV1::SignedNotSubmitted,
            intent_sha256: sha256_hex(
                &serde_json::to_vec(&intent).expect("serialize collateral intent"),
            ),
            envelope_sha256: String::new(),
            intent,
            signed_packet_base64: Some("durable packet".into()),
            signed_packet_sha256: Some("durable digest".into()),
            expected_signature: Some("durable signature".into()),
            finalized: None,
        };
        collateral.envelope_sha256 =
            collateral_envelope_digest(&collateral).expect("signed envelope");
        collateral.phase = CollateralPhaseV1::Planned;
        collateral.signed_packet_base64 = None;
        collateral.signed_packet_sha256 = None;
        collateral.expected_signature = None;
        assert_ne!(
            collateral.envelope_sha256,
            collateral_envelope_digest(&collateral).expect("reverted envelope")
        );
    }

    fn isolated_journal_path(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let directory = PathBuf::from(format!(
            "/private/tmp/dclutch-participant-journal-{}-{nonce}-{label}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("isolated journal directory");
        directory.join("report.json")
    }

    #[test]
    fn journal_no_replace_and_stale_writer_cas_never_clobber() {
        let path = isolated_journal_path("cas");
        let directory = path.parent().expect("journal parent").to_path_buf();
        let first = b"first\n".to_vec();
        let second = b"second\n".to_vec();
        let third = b"third\n".to_vec();
        let mut creator = ReportJournalV1::vacant(path.clone());
        let mut concurrent_creator = ReportJournalV1::vacant(path.clone());
        creator
            .persist_exact_bytes(first.clone())
            .expect("first no-replace create");
        assert!(
            concurrent_creator
                .persist_exact_bytes(second.clone())
                .is_err()
        );
        assert_eq!(fs::read(&path).expect("created journal"), first);

        let mut writer = ReportJournalV1::existing(path.clone(), first.clone());
        let mut stale = ReportJournalV1::existing(path.clone(), first);
        writer
            .persist_exact_bytes(second.clone())
            .expect("CAS update");
        assert!(stale.persist_exact_bytes(third).is_err());
        assert_eq!(fs::read(&path).expect("CAS journal"), second);
        fs::remove_dir_all(directory).expect("remove isolated journal directory");
    }

    #[test]
    fn journal_refuses_a_concurrent_lock_holder() {
        let path = isolated_journal_path("lock");
        let directory = path.parent().expect("journal parent").to_path_buf();
        let name = path.file_name().expect("journal name");
        let mut lock_name = OsString::from(".");
        lock_name.push(name);
        lock_name.push(".user-position.lock");
        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(lock_name))
            .expect("journal lock");
        held.try_lock().expect("hold journal lock");
        let mut journal = ReportJournalV1::vacant(path);
        assert!(journal.persist_exact_bytes(b"blocked\n".to_vec()).is_err());
        drop(held);
        fs::remove_dir_all(directory).expect("remove isolated journal directory");
    }

    #[test]
    fn participant_token_seed_binds_market_participant_and_release() {
        let market = Pubkey::new_unique();
        let participant = Pubkey::new_unique();
        let seed = participant_collateral_seed_v1(market, participant, [7; 32]);
        assert_eq!(seed.len(), 32);
        assert_eq!(
            seed,
            participant_collateral_seed_v1(market, participant, [7; 32])
        );
        assert_ne!(
            seed,
            participant_collateral_seed_v1(Pubkey::new_unique(), participant, [7; 32])
        );
        assert_ne!(
            seed,
            participant_collateral_seed_v1(market, Pubkey::new_unique(), [7; 32])
        );
        assert_ne!(
            seed,
            participant_collateral_seed_v1(market, participant, [8; 32])
        );
    }

    #[test]
    fn cli_refuses_relative_key_path_and_confirmed_escape_hatch() {
        let mut relative = base_args();
        let index = relative
            .iter()
            .position(|value| value == "--position-owner-keypair")
            .expect("flag");
        relative[index + 1] = "owner.json".into();
        assert!(
            parse_arguments(relative)
                .expect_err("relative")
                .to_string()
                .contains("must be absolute")
        );

        let mut confirmed = base_args();
        confirmed.extend(["--commitment".into(), "confirmed".into()]);
        assert!(
            parse_arguments(confirmed)
                .expect_err("confirmed")
                .to_string()
                .contains("unknown")
        );
    }

    #[test]
    fn cli_refuses_loopback_even_with_devnet_acknowledgment() {
        let mut args = base_args();
        args[1] = "http://127.0.0.1:20990".into();
        assert!(parse_arguments(args).is_err());
    }

    #[test]
    fn owned_loopback_report_is_not_a_public_devnet_report() {
        assert_eq!(
            report_schema_v1(ExpectedClusterV1::Devnet),
            "dclutch-devnet-user-position-admission-execution-v1"
        );
        assert_eq!(
            report_schema_v1(ExpectedClusterV1::OwnedLoopback),
            "dclutch-owned-loopback-user-position-admission-execution-v1"
        );
        assert_ne!(
            report_schema_v1(ExpectedClusterV1::Devnet),
            report_schema_v1(ExpectedClusterV1::OwnedLoopback)
        );
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FakeSignatureState {
        Missing,
        Confirmed,
        FinalizedOk,
        FinalizedError,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ResumeDecision {
        Sign,
        VerifyFinalized,
        RefuseAmbiguous,
        RefuseFailed,
        RefuseReplayWithoutSignature,
    }

    fn fake_collateral_resume_decision(
        phase: CollateralPhaseV1,
        signature: Option<FakeSignatureState>,
        exact_poststate: bool,
    ) -> ResumeDecision {
        if exact_poststate && signature.is_none() {
            return ResumeDecision::RefuseReplayWithoutSignature;
        }
        match signature {
            Some(FakeSignatureState::FinalizedOk) => ResumeDecision::VerifyFinalized,
            Some(FakeSignatureState::FinalizedError) => ResumeDecision::RefuseFailed,
            Some(FakeSignatureState::Missing | FakeSignatureState::Confirmed) => {
                ResumeDecision::RefuseAmbiguous
            }
            None if phase == CollateralPhaseV1::Planned => ResumeDecision::Sign,
            None => ResumeDecision::RefuseAmbiguous,
        }
    }

    #[test]
    fn collateral_phase_machine_never_resends_ambiguous_or_replays_exact_poststate() {
        assert_eq!(
            fake_collateral_resume_decision(CollateralPhaseV1::Planned, None, false),
            ResumeDecision::Sign
        );
        assert_eq!(
            fake_collateral_resume_decision(
                CollateralPhaseV1::SignedNotSubmitted,
                Some(FakeSignatureState::Missing),
                false,
            ),
            ResumeDecision::RefuseAmbiguous
        );
        assert_eq!(
            fake_collateral_resume_decision(
                CollateralPhaseV1::Submitted,
                Some(FakeSignatureState::Confirmed),
                false,
            ),
            ResumeDecision::RefuseAmbiguous
        );
        assert_eq!(
            fake_collateral_resume_decision(CollateralPhaseV1::Planned, None, true),
            ResumeDecision::RefuseReplayWithoutSignature
        );
        assert_eq!(
            fake_collateral_resume_decision(
                CollateralPhaseV1::Submitted,
                Some(FakeSignatureState::FinalizedError),
                false,
            ),
            ResumeDecision::RefuseFailed
        );
        assert_eq!(
            fake_collateral_resume_decision(
                CollateralPhaseV1::Submitted,
                Some(FakeSignatureState::FinalizedOk),
                true,
            ),
            ResumeDecision::VerifyFinalized
        );
    }

    #[test]
    fn dispatching_admission_recovery_has_one_exact_resend_permission() {
        assert_eq!(
            admission_recovery_v1(PhaseV1::Planned, None).expect("unsigned plan"),
            AdmissionRecoveryV1::SignOnce
        );
        assert_eq!(
            admission_recovery_v1(
                PhaseV1::Dispatching,
                Some(AdmissionSignatureStateV1::Absent),
            )
            .expect("pre-send restart"),
            AdmissionRecoveryV1::ResendIdenticalPacket
        );
        assert_eq!(
            admission_recovery_v1(
                PhaseV1::Dispatching,
                Some(AdmissionSignatureStateV1::Pending),
            )
            .expect("pending restart"),
            AdmissionRecoveryV1::PollOnly
        );
        assert_eq!(
            admission_recovery_v1(
                PhaseV1::Dispatching,
                Some(AdmissionSignatureStateV1::Finalized),
            )
            .expect("landed restart"),
            AdmissionRecoveryV1::Finalize
        );
        for historic in [PhaseV1::SignedNotSubmitted, PhaseV1::Submitted] {
            assert_eq!(
                admission_recovery_v1(historic, Some(AdmissionSignatureStateV1::Absent),)
                    .expect("historic absent packet"),
                AdmissionRecoveryV1::RefuseHistoricAmbiguity
            );
            assert_eq!(
                admission_recovery_v1(historic, Some(AdmissionSignatureStateV1::Pending),)
                    .expect("historic pending packet"),
                AdmissionRecoveryV1::PollOnly
            );
            assert_eq!(
                admission_recovery_v1(historic, Some(AdmissionSignatureStateV1::Finalized),)
                    .expect("historic landed packet"),
                AdmissionRecoveryV1::Finalize
            );
        }
        for phase in [
            PhaseV1::Planned,
            PhaseV1::SignedNotSubmitted,
            PhaseV1::Submitted,
            PhaseV1::Finalized,
        ] {
            assert_ne!(
                admission_recovery_v1(phase, Some(AdmissionSignatureStateV1::Absent),).ok(),
                Some(AdmissionRecoveryV1::ResendIdenticalPacket),
                "only Dispatching plus an absent exact signature can send"
            );
        }
    }

    #[test]
    fn dispatching_envelope_refuses_packet_and_signature_substitution() {
        let (mut report, _) = admission_exactness_fixture();
        report.phase = PhaseV1::Dispatching;
        report.envelope_sha256 = admission_envelope_digest(&report).expect("dispatching envelope");
        authenticate_report_phase_envelopes(&report).expect("exact Dispatching packet");

        let mut packet_substitution = report.clone();
        packet_substitution.signed_packet_sha256 = Some(hex(&[0x61; 32]));
        packet_substitution.envelope_sha256 =
            admission_envelope_digest(&packet_substitution).expect("attacker envelope");
        assert!(authenticate_report_phase_envelopes(&packet_substitution).is_err());

        let mut signature_substitution = report;
        signature_substitution.expected_signature = Some(Signature::from([0x62; 64]).to_string());
        signature_substitution.envelope_sha256 =
            admission_envelope_digest(&signature_substitution).expect("attacker envelope");
        assert!(authenticate_report_phase_envelopes(&signature_substitution).is_err());
    }

    #[test]
    fn historic_signed_and_submitted_report_phases_remain_authenticatable() {
        let (mut report, _) = admission_exactness_fixture();
        for phase in [PhaseV1::SignedNotSubmitted, PhaseV1::Submitted] {
            report.phase = phase;
            report.envelope_sha256 =
                admission_envelope_digest(&report).expect("historic phase envelope");
            let encoded = serde_json::to_vec(&report).expect("historic report JSON");
            let reopened: ReportV1 = serde_json::from_slice(&encoded).expect("historic report");
            assert_eq!(reopened.phase, phase);
            authenticate_report_phase_envelopes(&reopened)
                .expect("historic signed packet remains authenticatable");
            assert_eq!(
                admission_recovery_v1(reopened.phase, Some(AdmissionSignatureStateV1::Absent),)
                    .expect("historic recovery"),
                AdmissionRecoveryV1::RefuseHistoricAmbiguity
            );
        }
    }
}
