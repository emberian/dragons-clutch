//! Typed activity projections and one-boundary finalized capture for a fresh
//! supervisor-owned validator.
//!
//! Raw lifecycle artifacts do not share one completion vocabulary.  This
//! module therefore has hard-coded adapters for each accepted semantic owner;
//! a caller may identify source paths, but may not supply account, delta, or
//! economic projections.  The normalized stage wrapper is the only source the
//! aggregate activity manifest consumes.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Component, Path, PathBuf},
    str::FromStr as _,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_direct_codec::fee_settlement_v1::DirectFeeSettlementReceiptV1;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};
use solana_loader_v3_interface::get_program_data_address;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Error, Result,
    aggregate_retirement_journal::{
        AGGREGATE_RETIREMENT_COMPLETION_SCHEMA_V1, AggregateRetirementConservationReceiptV1,
        AggregateRetirementOperationV1, authenticate_aggregate_retirement_conservation_receipt_v1,
    },
    campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1,
    cluster::{ClusterOriginV1, DEVNET_GENESIS_HASH, ExpectedClusterV1, MAINNET_BETA_GENESIS_HASH},
    direct_trade::{
        AuthenticatedDirectTradeEvidenceV1, authenticate_owned_loopback_terminal_evidence_v1,
    },
    local_mutable::authenticate_checked_local_mutable_plan_v1,
    model::SuccessorPlan,
    rpc::{Rpc, WritePolicyV1, parse_json_without_duplicate_keys_v1},
    user_position_admission::parse_finalized_direct_participant_evidence_v1,
    wallet_terminal::{LookupTableRequirementV1, PlanInputV1, SelectedInputV1},
};

pub(crate) const STAGE_COMMAND_V1: &str = "local-private-validator-activity-stage-completion-v1";
pub(crate) const MANIFEST_COMMAND_V1: &str = "local-private-validator-activity-manifest-v1";
pub(crate) const CAPTURE_COMMAND_V1: &str = "local-private-validator-finalized-activity-capture-v1";
pub(crate) const LIFECYCLE_SESSION_COMMAND_V1: &str =
    "local-private-validator-lifecycle-session-v1";

const STAGE_SCHEMA_V1: &str = "dclutch-owned-loopback-activity-stage-completion-v1";
const SOURCE_DESCRIPTOR_SCHEMA_V1: &str = "dclutch-owned-loopback-activity-stage-sources-v1";
const MANIFEST_SCHEMA_V1: &str = "dclutch-owned-loopback-activity-reconcile-manifest-v1";
const CAPTURE_SCHEMA_V1: &str = "dclutch-owned-loopback-captured-finalized-rpc-v1";
const JOURNAL_DESCRIPTORS_SCHEMA_V1: &str = "dclutch-owned-loopback-stage-journal-descriptors-v1";
const LIFECYCLE_SESSION_SCHEMA_V1: &str = "dclutch-owned-loopback-private-lifecycle-session-v1";
const DIRECT_OWNED_JOURNAL_SCHEMA_V1: &str = "dclutch-owned-loopback-direct-trade-journal-v1";
const CAMPAIGN_SCHEMA_V1: &str = "dclutch-successor-campaign-report-v1";
const PARTICIPANT_SCHEMA_V1: &str = "dclutch-owned-loopback-user-position-admission-execution-v1";
const DIRECT_EVIDENCE_SCHEMA_V1: &str = "dclutch-owned-loopback-direct-trade-finalized-v1";
const DIRECT_FEE_SETTLEMENT_SCHEMA_V1: &str = "dclutch-direct-fee-settlement-evidence-v1";
const RESOLUTION_INPUT_FORMAT_V1: &str = "dclutch-owned-loopback-flagship-resolution-input-v1";
const RESOLUTION_CHECKPOINT_FORMAT_V1: &str =
    "dclutch-owned-loopback-flagship-resolution-checkpoint-v3";
const PAYOUT_INPUT_FORMAT_V1: &str = "dclutch-wallet-terminal-payout-plan-input-v1";
const PAYOUT_EVIDENCE_SCHEMA_V1: &str =
    "dclutch-local-private-validator-wallet-terminal-payout-evidence-v1";
const PYTH_RECEIVER_PROGRAM_ID: &str = "rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp";
const PYTH_ROUTER_PROGRAM_ID: &str = "HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL";
const MAX_JSON_BYTES: u64 = 32 * 1024 * 1024;
const MAX_EVENTS: usize = 128;
const MAX_ACCOUNTS: usize = 512;
const STAGES: [&str; 6] = [
    "founding",
    "participant",
    "direct",
    "resolution",
    "payout",
    "retirement",
];
const LIFECYCLE_SESSION_STAGES: [&str; 8] = [
    "founding",
    "participant",
    "alt",
    "seal",
    "direct",
    "resolution",
    "payout",
    "retirement",
];
const FOUNDING_SUCCESS_MUTATIONS: [&str; 6] = [
    "prepare exact controller funding ledgers and checkpoint (DCLTCFQ1)",
    "stage projected custody against prepared controller funding (DCLTPCB2)",
    "found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF3)",
    "core-funding-create-v1",
    "resolution-funding-activate-v1",
    "core-funding-accept-v1",
];

#[derive(Clone, Debug)]
pub(crate) struct StageArgumentsV1 {
    rpc_url: String,
    stage: String,
    evidence_root: PathBuf,
    source_descriptors: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct ManifestArgumentsV1 {
    rpc_url: String,
    plan: PathBuf,
    stage_journal_descriptors: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct CaptureArgumentsV1 {
    rpc_url: String,
    plan: PathBuf,
    activity_manifest: PathBuf,
    stage_journal_descriptors: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct LifecycleSessionArgumentsV1 {
    rpc_url: String,
    evidence_root: PathBuf,
    stage_journal_descriptors: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDescriptorDocumentV1 {
    schema: String,
    stage: String,
    sources: Vec<SourceDescriptorV1>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDescriptorV1 {
    role: String,
    path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalDescriptorDocumentV1 {
    schema: String,
    journals: Vec<JournalDescriptorV1>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JournalDescriptorV1 {
    semantic_role: String,
    path: String,
    schema: String,
    completion_pointer: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceRefV1 {
    role: String,
    path: String,
    sha256: String,
    schema: String,
    completion_pointer: String,
    completion_value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountV1 {
    r#ref: String,
    address: String,
    kind: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    asset_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    program_owner: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DeltaV1 {
    account: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lamports: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    atoms: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EventV1 {
    id: String,
    kind: String,
    operation: String,
    predecessor: Option<String>,
    signature: String,
    slot: String,
    fee_payer: String,
    fee_lamports: String,
    compute_units_consumed: String,
    lamport_deltas: Vec<DeltaV1>,
    token_deltas: Vec<DeltaV1>,
    source_path: String,
    source_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    direct: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    positions: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fee_settlement: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    certificate: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payout: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retirement: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalAccountV1 {
    account: String,
    closed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lamports: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount_atoms: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StageCompletionV1 {
    schema: String,
    stage: String,
    status: String,
    cluster: String,
    genesis_hash: String,
    sources: Vec<SourceRefV1>,
    accounts: Vec<AccountV1>,
    events: Vec<EventV1>,
    final_accounts: Vec<FinalAccountV1>,
    projection_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestV1 {
    schema: String,
    activity_id: String,
    cluster: Value,
    accounts: Vec<AccountV1>,
    events: Vec<EventV1>,
    final_accounts: Vec<FinalAccountV1>,
    source_set_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LifecycleSessionStageV1 {
    stage: String,
    path: String,
    sha256: String,
    schema: String,
    completion_pointer: String,
    completion_value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LifecycleSessionV1 {
    schema: String,
    status: String,
    cluster: String,
    genesis_hash: String,
    stages: Vec<LifecycleSessionStageV1>,
    completed_stages: Vec<String>,
    stage_set_sha256: String,
}

#[derive(Clone, Debug)]
struct AuthenticatedLifecycleDescriptorV1 {
    descriptor: JournalDescriptorV1,
    source_sha256: String,
    completion_value: String,
    mutation_kind: Option<String>,
    stage_completion: Option<StageCompletionV1>,
}

#[derive(Clone, Debug)]
struct RawSourceV1 {
    role: String,
    path: PathBuf,
    relative: String,
    bytes: Vec<u8>,
    value: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TokenIdentityV1 {
    mint: String,
    authority: String,
    program_owner: String,
}

#[derive(Clone, Debug)]
struct PendingEventV1 {
    operation: String,
    signature: String,
    expected_slot: u64,
    expected_fee: u64,
    expected_compute: Option<u64>,
    source_path: String,
    source_sha256: String,
    direct: Option<Value>,
    positions: Option<Value>,
    fee_settlement: Option<Value>,
    forbidden_fee_payers: Vec<String>,
    required_transaction_accounts: Vec<String>,
    direct_fee_receipt: Option<DirectFeeReceiptExpectationV1>,
    expected_return_data: Option<ExpectedReturnDataV1>,
    expected_fee_payer: Option<String>,
    position: Option<Value>,
    certificate: Option<Value>,
    payout: Option<Value>,
    retirement: Option<Value>,
}

#[derive(Clone, Debug)]
struct DirectFeeReceiptExpectationV1 {
    producer: String,
    market: String,
    maker: String,
    maker_replay: String,
    fee_source: String,
    fee_destination: String,
    fee_recipient: String,
    settled_amount: u64,
    expected_revision: u64,
    resulting_revision: u64,
}

#[derive(Clone, Debug)]
struct ExpectedReturnDataV1 {
    producer: String,
    body: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectFeeSettlementEvidenceV1 {
    schema: String,
    cluster: String,
    market: String,
    generation: u64,
    maker: String,
    maker_replay: String,
    fee_owed: u64,
    fee_source: String,
    fee_destination: String,
    fee_destination_owner: String,
    standing_allowance: u64,
    caller_authority: String,
    caller_authority_bump: u8,
    custody_expected_revision: u64,
    custody_resulting_revision: u64,
    landed: Option<DirectFeeSettlementFinalizationV1>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectFeeSettlementFinalizationV1 {
    signature: String,
    slot: u64,
    compute_units_consumed: Option<u64>,
    fee_lamports: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PayoutObservedAccountV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_len: usize,
    data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PayoutEvidenceV1 {
    schema: String,
    cluster: String,
    input_sha256: String,
    payout_intent_sha256: String,
    journal_state_sha256: String,
    signature: String,
    finalized_slot: u64,
    fee_lamports: u64,
    compute_units_consumed: u64,
    fee_payer: String,
    owner: String,
    market: String,
    recipient: String,
    payout: String,
    lookup_table: String,
    lookup_addresses_sha256: String,
    payout_instruction_sha256: String,
    custody_request_sha256: Option<String>,
    return_data_producer: String,
    return_data_base64: String,
    poststates: Vec<PayoutObservedAccountV1>,
    evidence_sha256: String,
}

#[derive(Clone, Debug)]
struct RpcEventV1 {
    event: EventV1,
    transaction: Value,
    token_identities: BTreeMap<String, TokenIdentityV1>,
    changed_addresses: BTreeSet<String>,
}

pub(crate) fn usage() -> &'static str {
    "\n  local-private-validator-activity-stage-completion-v1 --rpc-url LOOPBACK --stage founding|participant|direct|resolution|payout|retirement --evidence-root ABS_DIR --source-descriptors ABS_JSON --output ABS_NEW\n  local-private-validator-activity-manifest-v1 --rpc-url LOOPBACK --plan ABS_JSON --stage-journal-descriptors ABS_JSON --output ABS_NEW\n  local-private-validator-finalized-activity-capture-v1 --rpc-url LOOPBACK --plan ABS_JSON --activity-manifest ABS_JSON --stage-journal-descriptors ABS_JSON --output ABS_NEW\n  local-private-validator-lifecycle-session-v1 --rpc-url LOOPBACK --evidence-root ABS_DIR --stage-journal-descriptors ABS_JSON --output ABS_NEW\n"
}

pub(crate) fn parse_stage_args<I>(arguments: I) -> Result<StageArgumentsV1>
where
    I: IntoIterator<Item = String>,
{
    let values = parse_flags(arguments)?;
    reject_unknown_flags(
        &values,
        &[
            "--rpc-url",
            "--stage",
            "--evidence-root",
            "--source-descriptors",
            "--output",
        ],
    )?;
    Ok(StageArgumentsV1 {
        rpc_url: required_flag(&values, "--rpc-url")?,
        stage: required_flag(&values, "--stage")?,
        evidence_root: PathBuf::from(required_flag(&values, "--evidence-root")?),
        source_descriptors: PathBuf::from(required_flag(&values, "--source-descriptors")?),
        output: PathBuf::from(required_flag(&values, "--output")?),
    })
}

pub(crate) fn parse_manifest_args<I>(arguments: I) -> Result<ManifestArgumentsV1>
where
    I: IntoIterator<Item = String>,
{
    let values = parse_flags(arguments)?;
    reject_unknown_flags(
        &values,
        &[
            "--rpc-url",
            "--plan",
            "--stage-journal-descriptors",
            "--output",
        ],
    )?;
    Ok(ManifestArgumentsV1 {
        rpc_url: required_flag(&values, "--rpc-url")?,
        plan: PathBuf::from(required_flag(&values, "--plan")?),
        stage_journal_descriptors: PathBuf::from(required_flag(
            &values,
            "--stage-journal-descriptors",
        )?),
        output: PathBuf::from(required_flag(&values, "--output")?),
    })
}

pub(crate) fn parse_capture_args<I>(arguments: I) -> Result<CaptureArgumentsV1>
where
    I: IntoIterator<Item = String>,
{
    let values = parse_flags(arguments)?;
    reject_unknown_flags(
        &values,
        &[
            "--rpc-url",
            "--plan",
            "--activity-manifest",
            "--stage-journal-descriptors",
            "--output",
        ],
    )?;
    Ok(CaptureArgumentsV1 {
        rpc_url: required_flag(&values, "--rpc-url")?,
        plan: PathBuf::from(required_flag(&values, "--plan")?),
        activity_manifest: PathBuf::from(required_flag(&values, "--activity-manifest")?),
        stage_journal_descriptors: PathBuf::from(required_flag(
            &values,
            "--stage-journal-descriptors",
        )?),
        output: PathBuf::from(required_flag(&values, "--output")?),
    })
}

pub(crate) fn parse_lifecycle_session_args<I>(arguments: I) -> Result<LifecycleSessionArgumentsV1>
where
    I: IntoIterator<Item = String>,
{
    let values = parse_flags(arguments)?;
    reject_unknown_flags(
        &values,
        &[
            "--rpc-url",
            "--evidence-root",
            "--stage-journal-descriptors",
            "--output",
        ],
    )?;
    Ok(LifecycleSessionArgumentsV1 {
        rpc_url: required_flag(&values, "--rpc-url")?,
        evidence_root: PathBuf::from(required_flag(&values, "--evidence-root")?),
        stage_journal_descriptors: PathBuf::from(required_flag(
            &values,
            "--stage-journal-descriptors",
        )?),
        output: PathBuf::from(required_flag(&values, "--output")?),
    })
}

fn parse_flags<I>(arguments: I) -> Result<BTreeMap<String, String>>
where
    I: IntoIterator<Item = String>,
{
    let mut out = BTreeMap::new();
    let mut values = arguments.into_iter();
    while let Some(flag) = values.next() {
        if !flag.starts_with("--") {
            return Err(Error::new(format!("unknown positional argument {flag}")));
        }
        let value = values
            .next()
            .ok_or_else(|| Error::new(format!("{flag} omitted its value")))?;
        if out.insert(flag.clone(), value).is_some() {
            return Err(Error::new(format!("{flag} may be supplied only once")));
        }
    }
    Ok(out)
}

fn required_flag(values: &BTreeMap<String, String>, flag: &str) -> Result<String> {
    values
        .get(flag)
        .cloned()
        .ok_or_else(|| Error::new(format!("{flag} is required")))
}

fn reject_unknown_flags(values: &BTreeMap<String, String>, allowed: &[&str]) -> Result<()> {
    if let Some(flag) = values.keys().find(|flag| !allowed.contains(&flag.as_str())) {
        return Err(Error::new(format!("unknown flag {flag}")));
    }
    Ok(())
}

pub(crate) fn run_lifecycle_session(arguments: LifecycleSessionArgumentsV1) -> Result<Value> {
    let origin = owned_loopback_origin(&arguments.rpc_url)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let genesis = owned_genesis(&mut rpc)?;
    let root = canonical_directory(&arguments.evidence_root, "lifecycle evidence root")?;
    let descriptor_path = canonical_regular(
        &arguments.stage_journal_descriptors,
        "lifecycle stage journal descriptors",
    )?;
    require_descendant(
        &root,
        &descriptor_path,
        "lifecycle stage journal descriptors",
    )?;
    let descriptors: JournalDescriptorDocumentV1 = exact_deserialize(
        &bounded_read(&descriptor_path, "lifecycle stage journal descriptors")?,
        "lifecycle stage journal descriptors",
    )?;
    if descriptors.schema != JOURNAL_DESCRIPTORS_SCHEMA_V1
        || descriptors.journals.is_empty()
        || descriptors.journals.len() > 160
    {
        return Err(Error::new(
            "lifecycle stage journal descriptors are another schema or outside the 1..160 bound",
        ));
    }
    let mut paths = BTreeSet::new();
    let mut roles = BTreeSet::new();
    let mut authenticated = Vec::with_capacity(descriptors.journals.len());
    for descriptor in descriptors.journals {
        if descriptor.semantic_role.is_empty()
            || descriptor.schema.is_empty()
            || !paths.insert(descriptor.path.clone())
            || !roles.insert(descriptor.semantic_role.clone())
        {
            return Err(Error::new(
                "lifecycle stage journal descriptors repeat a path/role or contain an empty identity",
            ));
        }
        if descriptor.schema == STAGE_SCHEMA_V1 {
            authenticate_normalized_stage_descriptor(&descriptor)?;
        }
        let path = resolve_relative(&root, &descriptor.path, "lifecycle journal source")?;
        let bytes = bounded_read(&path, "lifecycle journal source")?;
        let value = parse_json_without_duplicate_keys_v1(&bytes)?;
        let completion_value = normalized_scalar(json_pointer(
            &value,
            &descriptor.completion_pointer,
            "lifecycle completion pointer",
        )?)?;
        if value.get("schema").and_then(Value::as_str) != Some(descriptor.schema.as_str())
            || completion_value != "finalized"
        {
            return Err(Error::new(
                "lifecycle journal source is another schema or not exactly finalized",
            ));
        }
        let mutation_kind = value
            .get("stage")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if matches!(descriptor.semantic_role.as_str(), "alt" | "seal") {
            let expected_kind = if descriptor.semantic_role == "alt" {
                "lookup-freeze"
            } else {
                "capability-seal"
            };
            if descriptor.schema != DIRECT_OWNED_JOURNAL_SCHEMA_V1
                || descriptor.completion_pointer != "/phase"
                || mutation_kind.as_deref() != Some(expected_kind)
            {
                return Err(Error::new(
                    "lifecycle alt/seal descriptor is not its exact finalized Direct journal",
                ));
            }
        }
        let stage_completion = if descriptor.schema == STAGE_SCHEMA_V1 {
            let completion: StageCompletionV1 = serde_json::from_value(value)?;
            authenticate_stage_completion(
                &mut rpc,
                &root,
                &descriptor.semantic_role,
                &genesis,
                &completion,
            )?;
            Some(completion)
        } else {
            None
        };
        authenticated.push(AuthenticatedLifecycleDescriptorV1 {
            descriptor,
            source_sha256: sha256(&bytes),
            completion_value,
            mutation_kind,
            stage_completion,
        });
    }
    let session = assemble_lifecycle_session(&genesis, &authenticated)?;
    if owned_genesis(&mut rpc)? != genesis {
        return Err(Error::new(
            "owned-loopback genesis changed while the lifecycle session was authenticated",
        ));
    }
    let value = serde_json::to_value(session)?;
    write_new_json(&arguments.output, &value)?;
    Ok(value)
}

fn assemble_lifecycle_session(
    expected_genesis: &str,
    descriptors: &[AuthenticatedLifecycleDescriptorV1],
) -> Result<LifecycleSessionV1> {
    parse_pubkey(expected_genesis, "lifecycle session genesis")?;
    if expected_genesis == DEVNET_GENESIS_HASH || expected_genesis == MAINNET_BETA_GENESIS_HASH {
        return Err(Error::new(
            "lifecycle session cannot bind a public cluster genesis",
        ));
    }
    let mut seen_paths = BTreeSet::new();
    let mut seen_roles = BTreeSet::new();
    let mut stage_rows = Vec::new();
    let mut completions = Vec::new();
    for row in descriptors {
        canonical_relative(&row.descriptor.path, "lifecycle session descriptor path")?;
        if row.descriptor.semantic_role.is_empty()
            || row.descriptor.schema.is_empty()
            || row.completion_value != "finalized"
            || !seen_paths.insert(row.descriptor.path.as_str())
            || !seen_roles.insert(row.descriptor.semantic_role.as_str())
        {
            return Err(Error::new(
                "lifecycle session descriptors are duplicate, empty, or provisional",
            ));
        }
        if row.source_sha256.len() != 64
            || !row
                .source_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || row
                .source_sha256
                .bytes()
                .any(|byte| byte.is_ascii_uppercase())
        {
            return Err(Error::new(
                "lifecycle session descriptor source digest is not lowercase SHA-256",
            ));
        }
        let session_stage =
            LIFECYCLE_SESSION_STAGES.contains(&row.descriptor.semantic_role.as_str());
        match &row.stage_completion {
            Some(completion) => {
                authenticate_normalized_stage_descriptor(&row.descriptor)?;
                if completion.schema != STAGE_SCHEMA_V1
                    || completion.stage != row.descriptor.semantic_role
                    || completion.status != "finalized"
                    || completion.cluster != "owned-loopback"
                    || completion.genesis_hash != expected_genesis
                {
                    return Err(Error::new(
                        "lifecycle session stage wrapper changed stage, status, cluster, or genesis",
                    ));
                }
                stage_rows.push(LifecycleSessionStageV1 {
                    stage: row.descriptor.semantic_role.clone(),
                    path: row.descriptor.path.clone(),
                    sha256: row.source_sha256.clone(),
                    schema: row.descriptor.schema.clone(),
                    completion_pointer: row.descriptor.completion_pointer.clone(),
                    completion_value: row.completion_value.clone(),
                });
                completions.push(completion);
            }
            None if row.descriptor.schema == STAGE_SCHEMA_V1 => {
                return Err(Error::new(
                    "lifecycle session stage descriptor omitted its authenticated wrapper",
                ));
            }
            None if matches!(row.descriptor.semantic_role.as_str(), "alt" | "seal") => {
                let expected_kind = if row.descriptor.semantic_role == "alt" {
                    "lookup-freeze"
                } else {
                    "capability-seal"
                };
                if row.descriptor.schema != DIRECT_OWNED_JOURNAL_SCHEMA_V1
                    || row.descriptor.completion_pointer != "/phase"
                    || row.mutation_kind.as_deref() != Some(expected_kind)
                {
                    return Err(Error::new(
                        "lifecycle session alt/seal stage changed its Direct journal identity",
                    ));
                }
                stage_rows.push(LifecycleSessionStageV1 {
                    stage: row.descriptor.semantic_role.clone(),
                    path: row.descriptor.path.clone(),
                    sha256: row.source_sha256.clone(),
                    schema: row.descriptor.schema.clone(),
                    completion_pointer: row.descriptor.completion_pointer.clone(),
                    completion_value: row.completion_value.clone(),
                });
            }
            None if session_stage => {
                return Err(Error::new(
                    "lifecycle session named stage omitted its authenticated semantic owner",
                ));
            }
            None => {}
        }
    }
    let stage_names = stage_rows
        .iter()
        .map(|row| row.stage.as_str())
        .collect::<Vec<_>>();
    if stage_names != LIFECYCLE_SESSION_STAGES {
        return Err(Error::new(
            "lifecycle session omits or reorders the exact eight-stage private projection",
        ));
    }
    authenticate_lifecycle_event_order(&completions)?;
    authenticate_lifecycle_source_lineage(&completions)?;
    let stage_set_sha256 = sha256(&canonical_json(&serde_json::to_value(&stage_rows)?)?);
    Ok(LifecycleSessionV1 {
        schema: LIFECYCLE_SESSION_SCHEMA_V1.into(),
        status: "finalized".into(),
        cluster: "owned-loopback".into(),
        genesis_hash: expected_genesis.into(),
        stages: stage_rows,
        completed_stages: LIFECYCLE_SESSION_STAGES
            .into_iter()
            .map(str::to_owned)
            .collect(),
        stage_set_sha256,
    })
}

fn authenticate_lifecycle_source_lineage(completions: &[&StageCompletionV1]) -> Result<()> {
    let direct = completions
        .iter()
        .find(|completion| completion.stage == "direct")
        .ok_or_else(|| Error::new("lifecycle session omitted Direct completion"))?;
    let payout = completions
        .iter()
        .find(|completion| completion.stage == "payout")
        .ok_or_else(|| Error::new("lifecycle session omitted payout completion"))?;
    let direct_source = direct
        .sources
        .iter()
        .find(|source| source.role == "evidence")
        .ok_or_else(|| Error::new("Direct completion omitted its evidence source"))?;
    let payout_source = payout
        .sources
        .iter()
        .find(|source| source.role == "direct-evidence")
        .ok_or_else(|| Error::new("payout completion omitted its Direct evidence source"))?;
    if direct_source.path != payout_source.path
        || direct_source.sha256 != payout_source.sha256
        || direct_source.schema != DIRECT_EVIDENCE_SCHEMA_V1
        || payout_source.schema != DIRECT_EVIDENCE_SCHEMA_V1
    {
        return Err(Error::new(
            "lifecycle payout did not consume the exact Direct evidence authenticated by its Direct stage",
        ));
    }
    Ok(())
}

fn authenticate_lifecycle_event_order(completions: &[&StageCompletionV1]) -> Result<()> {
    if completions.len() != STAGES.len() {
        return Err(Error::new(
            "lifecycle session event closure omitted one of six stages",
        ));
    }
    let mut signatures = BTreeSet::new();
    let mut prior_slot = 0_u64;
    for (completion, expected_stage) in completions.iter().zip(STAGES) {
        if completion.stage != expected_stage || completion.events.is_empty() {
            return Err(Error::new(
                "lifecycle session event closure changed stage order or became empty",
            ));
        }
        for (index, event) in completion.events.iter().enumerate() {
            let expected_id = format!("{expected_stage}-{index:03}");
            let expected_predecessor = index
                .checked_sub(1)
                .map(|prior| format!("{expected_stage}-{prior:03}"));
            let slot = canonical_decimal(&event.slot, "lifecycle session event slot", true)?;
            Signature::from_str(&event.signature).map_err(|error| {
                Error::new(format!("lifecycle session event signature: {error}"))
            })?;
            if event.kind != expected_stage
                || event.id != expected_id
                || event.predecessor != expected_predecessor
                || slot < prior_slot
                || !signatures.insert(event.signature.as_str())
            {
                return Err(Error::new(
                    "lifecycle session mutations are reordered, duplicated, or misowned",
                ));
            }
            prior_slot = slot;
        }
    }
    Ok(())
}

pub(crate) fn run_stage(arguments: StageArgumentsV1) -> Result<Value> {
    require_frozen_stage_adapter(&arguments.stage)?;
    let root = canonical_directory(&arguments.evidence_root, "evidence root")?;
    let descriptor_path = canonical_regular(&arguments.source_descriptors, "source descriptors")?;
    require_descendant(&root, &descriptor_path, "source descriptors")?;
    let descriptors: SourceDescriptorDocumentV1 = exact_deserialize(
        &bounded_read(&descriptor_path, "source descriptors")?,
        "source descriptors",
    )?;
    if descriptors.schema != SOURCE_DESCRIPTOR_SCHEMA_V1 || descriptors.stage != arguments.stage {
        return Err(Error::new("source descriptors are another schema or stage"));
    }
    let sources = load_stage_sources(&root, &arguments.stage, descriptors.sources)?;
    let origin = owned_loopback_origin(&arguments.rpc_url)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let genesis_hash = owned_genesis(&mut rpc)?;
    let completion = derive_stage_completion(&arguments.stage, &sources, &mut rpc, &genesis_hash)?;
    let value = serde_json::to_value(completion)?;
    write_new_json(&arguments.output, &value)?;
    Ok(value)
}

fn require_frozen_stage_adapter(stage: &str) -> Result<()> {
    match stage {
        "founding" | "participant" | "direct" | "resolution" | "payout" | "retirement" => Ok(()),
        _ => Err(Error::new(
            "activity stage is not one of the exact six public kinds",
        )),
    }
}

fn derive_stage_completion(
    stage: &str,
    sources: &[RawSourceV1],
    rpc: &mut Rpc,
    genesis_hash: &str,
) -> Result<StageCompletionV1> {
    require_frozen_stage_adapter(stage)?;
    let (source_refs, pending, kind_overrides) = match stage {
        "founding" => adapt_founding(sources)?,
        "participant" => adapt_participant(sources, rpc)?,
        "direct" => adapt_direct(sources, rpc)?,
        "resolution" => adapt_resolution(sources)?,
        "payout" => adapt_payout(sources, rpc)?,
        "retirement" => adapt_retirement(sources)?,
        _ => unreachable!("stage adapter vocabulary was checked above"),
    };
    let mut rpc_events = Vec::with_capacity(pending.len());
    for pending in pending {
        rpc_events.push(reopen_event(rpc, stage, pending)?);
    }
    let (accounts, final_accounts, mut events) =
        project_accounts(rpc, stage, rpc_events, kind_overrides)?;
    for (index, event) in events.iter_mut().enumerate() {
        event.id = format!("{stage}-{index:03}");
        event.predecessor = index
            .checked_sub(1)
            .map(|prior| format!("{stage}-{prior:03}"));
    }
    let projection = json!({
        "accounts": accounts,
        "events": events,
        "finalAccounts": final_accounts,
    });
    Ok(StageCompletionV1 {
        schema: STAGE_SCHEMA_V1.into(),
        stage: stage.into(),
        status: "finalized".into(),
        cluster: "owned-loopback".into(),
        genesis_hash: genesis_hash.into(),
        sources: source_refs,
        accounts: serde_json::from_value(projection["accounts"].clone())?,
        events: serde_json::from_value(projection["events"].clone())?,
        final_accounts: serde_json::from_value(projection["finalAccounts"].clone())?,
        projection_sha256: sha256(&canonical_json(&projection)?),
    })
}

fn load_stage_sources(
    root: &Path,
    stage: &str,
    descriptors: Vec<SourceDescriptorV1>,
) -> Result<Vec<RawSourceV1>> {
    let roles = descriptors
        .iter()
        .map(|row| row.role.as_str())
        .collect::<Vec<_>>();
    validate_source_roles(stage, &roles)?;
    let mut paths = BTreeSet::new();
    descriptors
        .into_iter()
        .map(|descriptor| {
            canonical_relative(&descriptor.path, "stage source path")?;
            if !paths.insert(descriptor.path.clone()) {
                return Err(Error::new("stage source descriptors repeat a path"));
            }
            let path = resolve_relative(root, &descriptor.path, "stage source")?;
            let bytes = bounded_read(&path, "stage source")?;
            let value = parse_json_without_duplicate_keys_v1(&bytes).map_err(|error| {
                Error::new(format!("stage source {}: {error}", descriptor.path))
            })?;
            if !value.is_object() {
                return Err(Error::new("stage source must be one JSON object"));
            }
            Ok(RawSourceV1 {
                role: descriptor.role,
                path,
                relative: descriptor.path,
                bytes,
                value,
            })
        })
        .collect()
}

fn validate_raw_source_roles(stage: &str, sources: &[RawSourceV1]) -> Result<()> {
    let roles = sources
        .iter()
        .map(|row| row.role.as_str())
        .collect::<Vec<_>>();
    validate_source_roles(stage, &roles)
}

fn validate_source_roles(stage: &str, roles: &[&str]) -> Result<()> {
    match stage {
        "founding" if roles != ["campaign"] => {
            return Err(Error::new("founding source roles must be exact [campaign]"));
        }
        "participant" if roles != ["admission"] => {
            return Err(Error::new(
                "participant source roles must be exact [admission]",
            ));
        }
        "direct" if roles != ["campaign", "evidence", "fee-settlement"] => {
            return Err(Error::new(
                "Direct source roles must be exact [campaign, evidence, fee-settlement]",
            ));
        }
        "resolution" if roles != ["input", "checkpoint"] => {
            return Err(Error::new(
                "resolution source roles must be exact [input, checkpoint]",
            ));
        }
        "retirement" if roles != ["completion"] => {
            return Err(Error::new(
                "retirement source roles must be exact [completion]",
            ));
        }
        "payout" => {
            if roles.len() < 3 || roles.len() > 65 || (roles.len() - 1) % 2 != 0 {
                return Err(Error::new(
                    "payout sources must be exact Direct evidence followed by one through 32 canonical input/evidence pairs",
                ));
            }
            if roles.first() != Some(&"direct-evidence") {
                return Err(Error::new(
                    "payout first source role must be exact direct-evidence",
                ));
            }
            for (index, pair) in roles[1..].chunks_exact(2).enumerate() {
                if pair != [format!("input-{index:03}"), format!("evidence-{index:03}")] {
                    return Err(Error::new(
                        "payout source roles are not canonical input-NNN/evidence-NNN pairs",
                    ));
                }
            }
        }
        "founding" | "participant" | "direct" | "resolution" | "retirement" => {}
        _ => {
            return Err(Error::new(
                "activity stage is outside the six-stage vocabulary",
            ));
        }
    }
    Ok(())
}

fn source_ref(
    source: &RawSourceV1,
    expected_schema: &str,
    schema_field: &str,
    completion_pointer: &str,
    expected_completion: &Value,
) -> Result<SourceRefV1> {
    if source.value.get(schema_field).and_then(Value::as_str) != Some(expected_schema) {
        return Err(Error::new(format!(
            "{} source {} is another schema",
            source.role, source.relative
        )));
    }
    let observed = json_pointer(&source.value, completion_pointer, &source.role)?;
    if observed != expected_completion {
        return Err(Error::new(format!(
            "{} source is provisional or carries another completion fact",
            source.role
        )));
    }
    Ok(SourceRefV1 {
        role: source.role.clone(),
        path: source.relative.clone(),
        sha256: sha256(&source.bytes),
        schema: expected_schema.into(),
        completion_pointer: completion_pointer.into(),
        completion_value: normalized_scalar(observed)?,
    })
}

fn adapt_founding(
    sources: &[RawSourceV1],
) -> Result<(
    Vec<SourceRefV1>,
    Vec<PendingEventV1>,
    BTreeMap<String, (String, String)>,
)> {
    let source = one_source(sources, "campaign")?;
    let source_ref = source_ref(
        source,
        CAMPAIGN_SCHEMA_V1,
        "schema",
        "/execution/completed",
        &Value::Bool(true),
    )?;
    parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &source.bytes,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    let execution = object(source.value.get("execution"), "campaign execution")?;
    if execution.get("recoveredFinalizedFounding") != Some(&Value::Bool(false)) {
        return Err(Error::new(
            "activity founding refuses a reconstructed or ambiguous campaign",
        ));
    }
    let rows = array(execution.get("transactions"), "campaign transactions")?;
    let mut pending = Vec::new();
    let mut labels = BTreeSet::new();
    let mut success_mutations = Vec::new();
    for row in rows {
        let row = object(Some(row), "campaign transaction")?;
        let label = string_field(row, "label", "campaign transaction")?;
        if !is_authorized_founding_label(label) {
            continue;
        }
        if !labels.insert(label.to_owned()) || row.get("error") != Some(&Value::Null) {
            return Err(Error::new(
                "authorized founding history repeats or failed an admitted mutation label",
            ));
        }
        if FOUNDING_SUCCESS_MUTATIONS.contains(&label) {
            success_mutations.push(label);
        }
        let fee = u64_field(row, "fee_lamports", "campaign transaction")?;
        ExpectedClusterV1::OwnedLoopback
            .authenticate_finalized_fee(fee, "owned-loopback founding mutation")?;
        let compute = optional_u64_field(row, "compute_units_consumed", "campaign transaction")?;
        if row.get("transaction_metadata_available") != Some(&Value::Bool(true))
            || compute == Some(0)
        {
            return Err(Error::new(
                "authorized founding mutation omitted finalized fee/compute metadata",
            ));
        }
        pending.push(PendingEventV1 {
            operation: label.into(),
            signature: signature_field(row, "signature", "campaign transaction")?,
            expected_slot: u64_field(row, "slot", "campaign transaction")?,
            expected_fee: fee,
            expected_compute: compute,
            source_path: source.relative.clone(),
            source_sha256: sha256(&source.bytes),
            direct: None,
            positions: None,
            fee_settlement: None,
            forbidden_fee_payers: Vec::new(),
            required_transaction_accounts: Vec::new(),
            direct_fee_receipt: None,
            expected_return_data: None,
            expected_fee_payer: None,
            position: None,
            certificate: None,
            payout: None,
            retirement: None,
        });
    }
    authenticate_founding_success_mutations(&success_mutations)?;
    let market = object(execution.get("market"), "campaign market")?;
    let account_rows = object(market.get("accounts"), "campaign market accounts")?;
    let mut kinds = BTreeMap::new();
    for (role, raw) in account_rows {
        let row = object(Some(raw), "campaign account")?;
        let address = pubkey_field(row, "address", "campaign account")?;
        let kind = if role.contains("position") {
            "position"
        } else if role.contains("wallet")
            || role.contains("vault")
            || role.contains("fixture_source")
            || role.contains("source_funder")
        {
            "token"
        } else {
            "protocol"
        };
        kinds.insert(address, (kind.into(), role.clone()));
    }
    Ok((vec![source_ref], pending, kinds))
}

fn is_authorized_founding_label(label: &str) -> bool {
    matches!(
        label,
        "initialize Core infrastructure profile"
            | "activate immutable release-set role: Core"
            | "activate immutable release-set role: Claims"
            | "activate immutable release-set role: Trading"
            | "activate immutable release-set role: Resolution"
            | "activate immutable release-set role: Custody"
            | "fund the founding principal supplier and its rent-capacity witness"
            | "create the founding generation's lifecycle RentCreditV2"
            | "prepare exact controller funding ledgers and checkpoint (DCLTCFQ1)"
            | "stage projected custody against prepared controller funding (DCLTPCB2)"
            | "pre-fund the founding's five program-allocated accounts"
            | "found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF3)"
            | "core-funding-create-v1"
            | "resolution-funding-activate-v1"
            | "core-funding-accept-v1"
    ) || label.starts_with("publish record: ")
        || label.starts_with("publish Product graph: ")
        || label.starts_with("create DCLTPCB2 routing address lookup table")
        || label.starts_with("extend DCLTPCB2 routing table page ")
        || label.starts_with("create DCLTGMF3 routing address lookup table")
        || label.starts_with("extend DCLTGMF3 routing table page ")
}

fn authenticate_founding_success_mutations(labels: &[&str]) -> Result<()> {
    if labels != FOUNDING_SUCCESS_MUTATIONS {
        return Err(Error::new(
            "founding history changed the exact DCLTCFQ1 -> DCLTPCB2 -> DCLTGMF3 -> CreateFund -> Activate -> Accept success order",
        ));
    }
    Ok(())
}

fn adapt_participant(
    sources: &[RawSourceV1],
    rpc: &mut Rpc,
) -> Result<(
    Vec<SourceRefV1>,
    Vec<PendingEventV1>,
    BTreeMap<String, (String, String)>,
)> {
    let source = one_source(sources, "admission")?;
    let source_ref = source_ref(
        source,
        PARTICIPANT_SCHEMA_V1,
        "schema",
        "/phase",
        &Value::String("finalized".into()),
    )?;
    let projection = parse_finalized_direct_participant_evidence_v1(&source.bytes, rpc)?;
    let report = object(Some(&source.value), "participant report")?;
    let finalized = object(report.get("finalized"), "participant finalized admission")?;
    let collateral = object(report.get("collateral"), "participant collateral")?;
    let collateral_finalized = object(
        collateral.get("finalized"),
        "participant finalized collateral",
    )?;
    let make = |operation: &str, row: &Map<String, Value>| -> Result<PendingEventV1> {
        Ok(PendingEventV1 {
            operation: operation.into(),
            signature: signature_field(row, "signature", operation)?,
            expected_slot: u64_field(row, "slot", operation)?,
            expected_fee: u64_field(row, "feeLamports", operation)?,
            expected_compute: optional_u64_field(row, "computeUnitsConsumed", operation)?,
            source_path: source.relative.clone(),
            source_sha256: sha256(&source.bytes),
            direct: None,
            positions: None,
            fee_settlement: None,
            forbidden_fee_payers: Vec::new(),
            required_transaction_accounts: Vec::new(),
            direct_fee_receipt: None,
            expected_return_data: None,
            expected_fee_payer: None,
            position: None,
            certificate: None,
            payout: None,
            retirement: None,
        })
    };
    let mut kinds = BTreeMap::from([
        (
            projection.position.to_string(),
            ("position".into(), "participant-position".into()),
        ),
        (
            projection.collateral_account.to_string(),
            ("token".into(), "participant-collateral".into()),
        ),
        (
            projection.owner.to_string(),
            ("wallet".into(), "participant-owner".into()),
        ),
    ]);
    let collateral_intent = object(collateral.get("intent"), "participant collateral intent")?;
    kinds.insert(
        pubkey_field(collateral_intent, "sourceAccount", "collateral source")?,
        ("token".into(), "participant-collateral-source".into()),
    );
    Ok((
        vec![source_ref],
        vec![
            make("admit participant position", finalized)?,
            make("prepare participant collateral", collateral_finalized)?,
        ],
        kinds,
    ))
}

fn adapt_direct(
    sources: &[RawSourceV1],
    rpc: &mut Rpc,
) -> Result<(
    Vec<SourceRefV1>,
    Vec<PendingEventV1>,
    BTreeMap<String, (String, String)>,
)> {
    let campaign_source = one_source(sources, "campaign")?;
    let evidence_source = one_source(sources, "evidence")?;
    let fee_source = one_source(sources, "fee-settlement")?;
    let campaign_ref = source_ref(
        campaign_source,
        CAMPAIGN_SCHEMA_V1,
        "schema",
        "/execution/completed",
        &Value::Bool(true),
    )?;
    let evidence_ref = source_ref(
        evidence_source,
        DIRECT_EVIDENCE_SCHEMA_V1,
        "schema",
        "/status",
        &Value::String("finalized".into()),
    )?;
    let fee_evidence: DirectFeeSettlementEvidenceV1 =
        exact_deserialize(&fee_source.bytes, "Direct fee-settlement evidence")?;
    let fee_finalization = fee_evidence
        .landed
        .as_ref()
        .ok_or_else(|| Error::new("Direct fee-settlement evidence is preflight-only"))?;
    let fee_ref = source_ref(
        fee_source,
        DIRECT_FEE_SETTLEMENT_SCHEMA_V1,
        "schema",
        "/landed/signature",
        &Value::String(fee_finalization.signature.clone()),
    )?;
    let campaign = parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_source.bytes,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    let market = campaign
        .accounts
        .get("market")
        .ok_or_else(|| Error::new("Direct activity campaign omitted its Market account"))?
        .address
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("Direct activity Market: {error}")))?;
    let authenticated = authenticate_owned_loopback_terminal_evidence_v1(
        rpc,
        &evidence_source.path,
        market,
        &campaign.plan_sha256,
        &campaign.market_sha256,
    )?;
    let authenticated = &authenticated.direct;
    let direct_evidence_object = object(Some(&evidence_source.value), "Direct evidence")?;
    let (direct_generation, trading_program) =
        embedded_direct_generation_and_program(direct_evidence_object)?;
    let (mut pending, mut kinds) = project_authenticated_direct_activity(
        authenticated,
        direct_evidence_object,
        &evidence_source.relative,
        &sha256(&evidence_source.bytes),
    )?;
    let fee_pending = project_authenticated_direct_fee_settlement(
        authenticated,
        direct_evidence_object,
        &fee_evidence,
        direct_generation,
        &trading_program,
        &fee_source.relative,
        &sha256(&fee_source.bytes),
    )?;
    kinds.insert(
        fee_evidence.maker_replay.clone(),
        ("protocol".into(), "direct-buyer-maker-replay".into()),
    );
    kinds.insert(
        fee_evidence.caller_authority.clone(),
        ("protocol".into(), "direct-fee-caller-authority".into()),
    );
    pending.push(fee_pending);
    Ok((vec![campaign_ref, evidence_ref, fee_ref], pending, kinds))
}

fn embedded_direct_generation_and_program(
    direct_evidence: &Map<String, Value>,
) -> Result<(u64, String)> {
    let encoded = string_field(direct_evidence, "publicManifestBase64", "Direct evidence")?;
    let bytes = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("Direct public manifest base64: {error}")))?;
    if BASE64.encode(&bytes) != encoded {
        return Err(Error::new("Direct public manifest base64 is noncanonical"));
    }
    let manifest = parse_json_without_duplicate_keys_v1(&bytes)?;
    let manifest = object(Some(&manifest), "Direct public manifest")?;
    let context = object(manifest.get("context"), "Direct public manifest context")?;
    let route = object(manifest.get("route"), "Direct public manifest route")?;
    let fixed = object(route.get("fixed"), "Direct public manifest fixed route")?;
    let generation = u64_field(context, "generation", "Direct public manifest context")?;
    let trading_program = pubkey_field(fixed, "tradingProgram", "Direct public manifest route")?;
    Ok((generation, trading_program))
}

/// Project only facts that the Direct semantic owner has already authenticated
/// from its embedded signed intents, finalized journals, chain history, and ten
/// live poststates. Setup/ALT/seal rows remain ordinary Direct events; the sole
/// Hot row owns the economic and Position transition facts.
fn project_authenticated_direct_activity(
    authenticated: &AuthenticatedDirectTradeEvidenceV1,
    evidence: &Map<String, Value>,
    source_path: &str,
    source_sha256: &str,
) -> Result<(Vec<PendingEventV1>, BTreeMap<String, (String, String)>)> {
    let fill_atoms = u64_field(evidence, "fillAtoms", "Direct evidence")?;
    let execution_price = u64_field(evidence, "executionPrice", "Direct evidence")?;
    let price_scale = u64_field(evidence, "priceScale", "Direct evidence")?;
    let fee_basis_points = u64_field(evidence, "feeBasisPointsPerSide", "Direct evidence")?;
    if fill_atoms == 0 || price_scale != 1_000_000 || fee_basis_points != 50 {
        return Err(Error::new(
            "authenticated Direct evidence changed its exact fill, scale, or 50-bps-per-side policy",
        ));
    }
    let positions = serde_json::to_value(&authenticated.positions)?;
    let direct = json!({
        "fillAtoms": fill_atoms.to_string(),
        "executionPrice": execution_price.to_string(),
        "priceScale": price_scale.to_string(),
        "feeBasisPointsPerSide": fee_basis_points.to_string(),
        "sellerToken": authenticated.seller_collateral_destination.to_string(),
        "buyerToken": authenticated.buyer_collateral_source.to_string(),
        "feeRecipientToken": authenticated.fee_token_account.to_string(),
        "mint": authenticated.mint.to_string(),
    });
    let mut pending = Vec::with_capacity(authenticated.mutations.len());
    for row in &authenticated.mutations {
        let hot = row.kind == "hot";
        pending.push(PendingEventV1 {
            operation: format!("direct-{}", row.kind),
            signature: row.signature.clone(),
            expected_slot: row.slot,
            expected_fee: row.fee_lamports,
            expected_compute: Some(row.compute_units_consumed),
            source_path: source_path.into(),
            source_sha256: source_sha256.into(),
            direct: hot.then(|| direct.clone()),
            positions: hot.then(|| positions.clone()),
            fee_settlement: None,
            forbidden_fee_payers: Vec::new(),
            required_transaction_accounts: Vec::new(),
            direct_fee_receipt: None,
            expected_return_data: None,
            expected_fee_payer: None,
            position: None,
            certificate: None,
            payout: None,
            retirement: None,
        });
    }
    if pending.is_empty()
        || pending
            .iter()
            .filter(|row| row.direct.is_some() || row.positions.is_some())
            .count()
            != 1
        || pending
            .last()
            .is_none_or(|row| row.direct.is_none() || row.positions.is_none())
    {
        return Err(Error::new(
            "authenticated Direct history did not project exactly one terminal Hot owner",
        ));
    }
    let kinds = BTreeMap::from([
        (
            authenticated.market.to_string(),
            ("protocol".into(), "direct-market".into()),
        ),
        (
            authenticated.seller_owner.to_string(),
            ("wallet".into(), "direct-seller-owner".into()),
        ),
        (
            authenticated.seller_position.to_string(),
            ("position".into(), "direct-seller-position".into()),
        ),
        (
            authenticated.buyer_owner.to_string(),
            ("wallet".into(), "direct-buyer-owner".into()),
        ),
        (
            authenticated.buyer_position.to_string(),
            ("position".into(), "direct-buyer-position".into()),
        ),
        (
            authenticated.buyer_collateral_source.to_string(),
            ("token".into(), "direct-buyer-collateral".into()),
        ),
        (
            authenticated.seller_collateral_destination.to_string(),
            ("token".into(), "direct-seller-collateral".into()),
        ),
        (
            authenticated.fee_token_account.to_string(),
            ("token".into(), "direct-fee-collateral".into()),
        ),
        (
            authenticated.fee_recipient.to_string(),
            ("wallet".into(), "direct-fee-recipient".into()),
        ),
    ]);
    Ok((pending, kinds))
}

fn project_authenticated_direct_fee_settlement(
    authenticated: &AuthenticatedDirectTradeEvidenceV1,
    direct_evidence: &Map<String, Value>,
    settlement: &DirectFeeSettlementEvidenceV1,
    direct_generation: u64,
    trading_program: &str,
    source_path: &str,
    source_sha256: &str,
) -> Result<PendingEventV1> {
    let fill_atoms = u128::from(u64_field(direct_evidence, "fillAtoms", "Direct evidence")?);
    let execution_price = u128::from(u64_field(
        direct_evidence,
        "executionPrice",
        "Direct evidence",
    )?);
    let price_scale = u128::from(u64_field(direct_evidence, "priceScale", "Direct evidence")?);
    let fee_basis_points = u128::from(u64_field(
        direct_evidence,
        "feeBasisPointsPerSide",
        "Direct evidence",
    )?);
    let product = fill_atoms
        .checked_mul(execution_price)
        .ok_or_else(|| Error::new("Direct fee-settlement gross overflowed"))?;
    if price_scale == 0 || product % price_scale != 0 {
        return Err(Error::new(
            "Direct fee settlement crosses an unnamed gross rounding boundary",
        ));
    }
    let gross = product / price_scale;
    let one_side = gross
        .checked_mul(fee_basis_points)
        .ok_or_else(|| Error::new("Direct fee-settlement side fee overflowed"))?
        / 10_000;
    let expected_fee = one_side
        .checked_mul(2)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| Error::new("Direct fee-settlement combined fee overflowed"))?;
    let finalization = settlement
        .landed
        .as_ref()
        .ok_or_else(|| Error::new("Direct fee-settlement evidence is preflight-only"))?;
    let expected_buyer_replay = authenticated
        .final_accounts
        .get(2)
        .ok_or_else(|| Error::new("authenticated Direct evidence omitted buyer maker replay"))?
        .address
        .as_str();
    for (value, label) in [
        (&settlement.market, "fee-settlement Market"),
        (&settlement.maker, "fee-settlement maker"),
        (&settlement.maker_replay, "fee-settlement maker replay"),
        (&settlement.fee_source, "fee-settlement source"),
        (&settlement.fee_destination, "fee-settlement destination"),
        (
            &settlement.fee_destination_owner,
            "fee-settlement destination owner",
        ),
        (
            &settlement.caller_authority,
            "fee-settlement caller authority",
        ),
    ] {
        parse_pubkey(value, label)?;
    }
    Signature::from_str(&finalization.signature)
        .map_err(|error| Error::new(format!("Direct fee-settlement signature: {error}")))?;
    let compute = finalization
        .compute_units_consumed
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::new("Direct fee settlement omitted positive compute units"))?;
    let fee_lamports = finalization
        .fee_lamports
        .ok_or_else(|| Error::new("Direct fee settlement omitted its finalized fee"))?;
    ExpectedClusterV1::OwnedLoopback
        .authenticate_finalized_fee(fee_lamports, "Direct fee settlement")?;
    if settlement.schema != DIRECT_FEE_SETTLEMENT_SCHEMA_V1
        || settlement.cluster != "owned-loopback"
        || settlement.market != authenticated.market.to_string()
        || settlement.generation != direct_generation
        || settlement.maker != authenticated.buyer_owner.to_string()
        || settlement.maker_replay != expected_buyer_replay
        || settlement.fee_owed != expected_fee
        || settlement.fee_source != authenticated.buyer_collateral_source.to_string()
        || settlement.fee_destination != authenticated.fee_token_account.to_string()
        || settlement.fee_destination_owner != authenticated.fee_recipient.to_string()
        || settlement.standing_allowance < expected_fee
        || settlement.custody_expected_revision.checked_add(1)
            != Some(settlement.custody_resulting_revision)
        || finalization.slot < authenticated.finalized_slot
    {
        return Err(Error::new(
            "Direct fee settlement changed its debtor, exact obligation, state-derived route, or finalized order",
        ));
    }
    Ok(PendingEventV1 {
        operation: "direct-fee-settlement".into(),
        signature: finalization.signature.clone(),
        expected_slot: finalization.slot,
        expected_fee: fee_lamports,
        expected_compute: Some(compute),
        source_path: source_path.into(),
        source_sha256: source_sha256.into(),
        direct: None,
        positions: None,
        fee_settlement: Some(json!({
            "generation": settlement.generation.to_string(),
            "debtor": settlement.maker,
            "makerReplay": settlement.maker_replay,
            "feeAtoms": settlement.fee_owed.to_string(),
            "sourceToken": settlement.fee_source,
            "destinationToken": settlement.fee_destination,
            "destinationOwner": settlement.fee_destination_owner,
            "callerAuthority": settlement.caller_authority,
            "callerAuthorityBump": settlement.caller_authority_bump.to_string(),
            "standingAllowanceAtoms": settlement.standing_allowance.to_string(),
            "custodyExpectedRevision": settlement.custody_expected_revision.to_string(),
            "custodyResultingRevision": settlement.custody_resulting_revision.to_string(),
            "submissionClass": "permissionless-state-derived-stranger",
            "capitalizationClass": "debtor-collateral-obligation-not-future-revenue-or-hoard",
        })),
        forbidden_fee_payers: vec![
            authenticated.seller_owner.to_string(),
            authenticated.buyer_owner.to_string(),
            authenticated.fee_recipient.to_string(),
        ],
        required_transaction_accounts: vec![
            settlement.market.clone(),
            settlement.maker_replay.clone(),
            settlement.fee_source.clone(),
            settlement.fee_destination.clone(),
            settlement.caller_authority.clone(),
            trading_program.into(),
        ],
        direct_fee_receipt: Some(DirectFeeReceiptExpectationV1 {
            producer: trading_program.into(),
            market: settlement.market.clone(),
            maker: settlement.maker.clone(),
            maker_replay: settlement.maker_replay.clone(),
            fee_source: settlement.fee_source.clone(),
            fee_destination: settlement.fee_destination.clone(),
            fee_recipient: settlement.fee_destination_owner.clone(),
            settled_amount: settlement.fee_owed,
            expected_revision: settlement.custody_expected_revision,
            resulting_revision: settlement.custody_resulting_revision,
        }),
        expected_return_data: None,
        expected_fee_payer: None,
        position: None,
        certificate: None,
        payout: None,
        retirement: None,
    })
}

fn adapt_resolution(
    sources: &[RawSourceV1],
) -> Result<(
    Vec<SourceRefV1>,
    Vec<PendingEventV1>,
    BTreeMap<String, (String, String)>,
)> {
    let input = one_source(sources, "input")?;
    let checkpoint = one_source(sources, "checkpoint")?;
    let input_ref = source_ref(
        input,
        RESOLUTION_INPUT_FORMAT_V1,
        "format",
        "/format",
        &Value::String(RESOLUTION_INPUT_FORMAT_V1.into()),
    )?;
    let checkpoint_ref = source_ref(
        checkpoint,
        RESOLUTION_CHECKPOINT_FORMAT_V1,
        "format",
        "/verifiedTerminal",
        &Value::Bool(true),
    )?;
    let input_object = object(Some(&input.value), "resolution input")?;
    let accounts = object(input_object.get("accounts"), "resolution input accounts")?;
    let certificate_address = pubkey_field(accounts, "certificate", "resolution certificate")?;
    let market = pubkey_field(accounts, "market", "resolution market")?;
    let receipts = array(checkpoint.value.get("receipts"), "resolution receipts")?;
    const RECEIPT_STAGES: [&str; 4] = [
        "submit",
        "resolution-provider-execute-v1",
        "core-terminal-accept-v1",
        "reclaim",
    ];
    const OPERATIONS: [&str; 4] = [
        "resolution-submit",
        "resolution-provider-execute-v1",
        "core-terminal-accept-v1",
        "resolution-reclaim",
    ];
    if receipts.len() != RECEIPT_STAGES.len() {
        return Err(Error::new(
            "terminal resolution requires exact submit, provider execute, Core accept, and reclaim receipts",
        ));
    }
    let mut pending = Vec::new();
    let mut signatures = BTreeSet::new();
    let mut prior_slot = None;
    for (index, ((raw, expected_stage), operation)) in receipts
        .iter()
        .zip(RECEIPT_STAGES)
        .zip(OPERATIONS)
        .enumerate()
    {
        let row = object(Some(raw), "resolution receipt")?;
        if row.get("stage").and_then(Value::as_str) != Some(expected_stage) {
            return Err(Error::new(
                "resolution receipts are not exact submit, provider execute, Core accept, then reclaim",
            ));
        }
        let signature = signature_field(row, "signature", "resolution receipt")?;
        let slot = u64_field(row, "slot", "resolution receipt")?;
        if !signatures.insert(signature.clone()) || prior_slot.is_some_and(|prior| prior >= slot) {
            return Err(Error::new(
                "resolution receipt signatures must be unique and slots strictly ordered",
            ));
        }
        prior_slot = Some(slot);
        pending.push(PendingEventV1 {
            operation: operation.into(),
            signature,
            expected_slot: slot,
            expected_fee: u64_field(row, "feeLamports", "resolution receipt")?,
            expected_compute: Some(u64_field(
                row,
                "computeUnitsConsumed",
                "resolution receipt",
            )?),
            source_path: checkpoint.relative.clone(),
            source_sha256: sha256(&checkpoint.bytes),
            direct: None,
            positions: None,
            fee_settlement: None,
            forbidden_fee_payers: Vec::new(),
            required_transaction_accounts: Vec::new(),
            direct_fee_receipt: None,
            expected_return_data: None,
            expected_fee_payer: None,
            position: None,
            certificate: (index == 1).then(|| {
                json!({
                    "account": certificate_address,
                    "market": market,
                    "_deriveFinalizedAccount": true,
                })
            }),
            payout: None,
            retirement: None,
        });
    }
    Ok((
        vec![input_ref, checkpoint_ref],
        pending,
        BTreeMap::from([
            (
                certificate_address,
                ("certificate".into(), "resolution-certificate".into()),
            ),
            (market, ("protocol".into(), "resolution-market".into())),
        ]),
    ))
}

fn adapt_payout(
    sources: &[RawSourceV1],
    rpc: &mut Rpc,
) -> Result<(
    Vec<SourceRefV1>,
    Vec<PendingEventV1>,
    BTreeMap<String, (String, String)>,
)> {
    let direct_source = sources
        .first()
        .filter(|source| source.role == "direct-evidence")
        .ok_or_else(|| Error::new("payout omitted its exact leading Direct evidence"))?;
    let direct_ref = source_ref(
        direct_source,
        DIRECT_EVIDENCE_SCHEMA_V1,
        "schema",
        "/status",
        &Value::String("finalized".into()),
    )?;
    let first_input = sources
        .get(1)
        .ok_or_else(|| Error::new("payout omitted its first input"))?;
    let first_input_value: PlanInputV1 = exact_deserialize(&first_input.bytes, "payout input")?;
    let first_selected =
        SelectedInputV1::parse(&first_input_value, LookupTableRequirementV1::Present)?;
    let direct_object = object(Some(&direct_source.value), "payout Direct evidence")?;
    let (manifest_market, plan_sha256, market_input_sha256) =
        embedded_direct_manifest_binding(direct_object)?;
    if manifest_market != first_selected.market {
        return Err(Error::new(
            "payout Direct evidence and first input name different Markets",
        ));
    }
    let terminal = authenticate_owned_loopback_terminal_evidence_v1(
        rpc,
        &direct_source.path,
        manifest_market,
        &plan_sha256,
        &market_input_sha256,
    )?;
    let authenticated = terminal.direct;
    let mut position_states = BTreeMap::new();
    for transition in &authenticated.positions {
        let account = parse_pubkey(&transition.account, "Direct payout Position")?;
        let owner = parse_pubkey(&transition.owner, "Direct payout Position owner")?;
        let post = decode_canonical_base64(
            &transition.post_data_base64,
            "Direct payout Position poststate",
        )?;
        if position_states.insert(account, (owner, post)).is_some() {
            return Err(Error::new(
                "Direct evidence repeats a payout Position transition",
            ));
        }
    }
    if position_states.len() != 2 {
        return Err(Error::new(
            "Direct evidence did not own the exact seller and buyer Position poststates",
        ));
    }

    let mut refs = Vec::with_capacity(sources.len());
    refs.push(direct_ref);
    let mut pending = Vec::with_capacity((sources.len() - 1) / 2);
    let mut kinds = BTreeMap::new();
    for (index, pair) in sources[1..].chunks_exact(2).enumerate() {
        let input = &pair[0];
        let evidence = &pair[1];
        let input_ref = source_ref(
            input,
            PAYOUT_INPUT_FORMAT_V1,
            "format",
            "/format",
            &Value::String(PAYOUT_INPUT_FORMAT_V1.into()),
        )?;
        let payout_evidence: PayoutEvidenceV1 =
            exact_deserialize(&evidence.bytes, "payout evidence")?;
        if payout_evidence.schema != PAYOUT_EVIDENCE_SCHEMA_V1
            || payout_evidence.cluster != "owned-loopback"
            || payout_evidence.evidence_sha256 != payout_evidence_digest(&payout_evidence)?
        {
            return Err(Error::new(
                "payout evidence schema, cluster, or self-digest changed",
            ));
        }
        let signature = payout_evidence.signature.clone();
        let evidence_ref = source_ref(
            evidence,
            PAYOUT_EVIDENCE_SCHEMA_V1,
            "schema",
            "/signature",
            &Value::String(signature.clone()),
        )?;
        let input_value: PlanInputV1 = exact_deserialize(&input.bytes, "payout input")?;
        let selected = SelectedInputV1::parse(&input_value, LookupTableRequirementV1::Present)?;
        if selected.market != manifest_market
            || sha256(&input.bytes) != payout_evidence.input_sha256
            || selected.market.to_string() != payout_evidence.market
            || selected.owner.to_string() != payout_evidence.owner
            || selected.recipient.to_string() != payout_evidence.recipient
        {
            return Err(Error::new(
                "payout input/evidence pair differs in bytes, Market, owner, or recipient",
            ));
        }
        let quantity = canonical_decimal(&input_value.quantity, "payout quantity", true)?;
        let principal = canonical_decimal(&payout_evidence.payout, "payout principal", false)?;
        let position_key = selected.position;
        let expected_holder = if position_key == authenticated.seller_position {
            authenticated.seller_owner
        } else if position_key == authenticated.buyer_position {
            authenticated.buyer_owner
        } else {
            return Err(Error::new(
                "payout input selected no Position authenticated by Direct history",
            ));
        };
        if selected.owner != expected_holder {
            return Err(Error::new(
                "payout input substituted the authenticated Direct Position owner",
            ));
        }
        let (position_owner, pre) = position_states
            .get(&position_key)
            .cloned()
            .ok_or_else(|| Error::new("payout repeats or omits a Direct Position state"))?;
        let (post, burns) = apply_position_payout(
            &pre,
            usize::try_from(input_value.claim_index)
                .map_err(|_| Error::new("payout claim index exceeds usize"))?,
            quantity,
        )?;
        authenticate_payout_position_poststate(
            &payout_evidence,
            position_key,
            position_owner,
            &post,
        )?;
        position_states.insert(position_key, (position_owner, post.clone()));
        let return_body = decode_canonical_base64(
            &payout_evidence.return_data_base64,
            "payout evidence return data",
        )?;
        parse_pubkey(
            &payout_evidence.return_data_producer,
            "payout return-data producer",
        )?;
        parse_pubkey(&payout_evidence.fee_payer, "payout fee payer")?;
        let position = position_key.to_string();
        let hoard = selected.hoard.to_string();
        let recipient = selected.recipient.to_string();
        let mint = input_value.collateral_mint.clone();
        let holder = selected.owner.to_string();
        pending.push(PendingEventV1 {
            operation: format!("wallet-terminal-payout-{index:03}"),
            signature,
            expected_slot: payout_evidence.finalized_slot,
            expected_fee: payout_evidence.fee_lamports,
            expected_compute: Some(payout_evidence.compute_units_consumed),
            source_path: evidence.relative.clone(),
            source_sha256: sha256(&evidence.bytes),
            direct: None,
            positions: None,
            fee_settlement: None,
            forbidden_fee_payers: vec![holder.clone()],
            required_transaction_accounts: vec![
                selected.aggregate.to_string(),
                position.clone(),
                hoard.clone(),
                recipient.clone(),
            ],
            direct_fee_receipt: None,
            expected_return_data: Some(ExpectedReturnDataV1 {
                producer: payout_evidence.return_data_producer,
                body: return_body,
            }),
            expected_fee_payer: Some(payout_evidence.fee_payer),
            position: None,
            certificate: None,
            payout: Some(json!({
                "_positionAddress": position,
                "_claimIndex": input_value.claim_index,
                "_quantity": quantity.to_string(),
                "_preDataBase64": BASE64.encode(&pre),
                "_postDataBase64": BASE64.encode(&post),
                "_positionOwner": position_owner.to_string(),
                "hoardToken": hoard,
                "recipientToken": recipient,
                "principalAtoms": principal.to_string(),
                "mint": mint,
                "holder": holder,
                "holderChargeClass": "terminal-holder-is-not-transaction-fee-payer",
                "_claimsBurnedAtoms": burns,
            })),
            retirement: None,
        });
        kinds.insert(
            position,
            ("position".into(), format!("payout-position-{index:03}")),
        );
        kinds.insert(hoard, ("token".into(), "hoard-principal".into()));
        kinds.insert(
            recipient,
            ("token".into(), format!("payout-recipient-{index:03}")),
        );
        kinds.insert(
            selected.owner.to_string(),
            ("wallet".into(), format!("payout-owner-{index:03}")),
        );
        refs.extend([input_ref, evidence_ref]);
    }
    Ok((refs, pending, kinds))
}

fn embedded_direct_manifest_binding(
    direct_evidence: &Map<String, Value>,
) -> Result<(Pubkey, String, String)> {
    let encoded = string_field(
        direct_evidence,
        "publicManifestBase64",
        "payout Direct evidence",
    )?;
    let bytes = decode_canonical_base64(encoded, "payout Direct public manifest")?;
    let manifest = parse_json_without_duplicate_keys_v1(&bytes)?;
    let manifest = object(Some(&manifest), "payout Direct public manifest")?;
    let market = parse_pubkey(
        string_field(manifest, "market", "payout Direct public manifest")?,
        "payout Direct public manifest Market",
    )?;
    let plan = exact_sha256_field(manifest, "planSha256", "payout Direct public manifest")?;
    let market_input = exact_sha256_field(
        manifest,
        "marketInputSha256",
        "payout Direct public manifest",
    )?;
    Ok((market, plan, market_input))
}

fn exact_sha256_field(value: &Map<String, Value>, key: &str, label: &str) -> Result<String> {
    let digest = string_field(value, key, label)?;
    if digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        || digest.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(Error::new(format!(
            "{label} {key} is not lowercase SHA-256"
        )));
    }
    Ok(digest.to_owned())
}

fn decode_canonical_base64(encoded: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("{label} base64: {error}")))?;
    if BASE64.encode(&bytes) != encoded {
        return Err(Error::new(format!("{label} base64 is noncanonical")));
    }
    Ok(bytes)
}

fn payout_evidence_digest(evidence: &PayoutEvidenceV1) -> Result<String> {
    let mut projected = evidence.clone();
    projected.evidence_sha256.clear();
    Ok(sha256(&serde_json::to_vec(&projected)?))
}

fn authenticate_payout_position_poststate(
    evidence: &PayoutEvidenceV1,
    position: Pubkey,
    owner: Pubkey,
    post: &[u8],
) -> Result<()> {
    if evidence.poststates.len() != 5 {
        return Err(Error::new("payout evidence poststate width changed"));
    }
    let mut addresses = BTreeSet::new();
    for row in &evidence.poststates {
        parse_pubkey(&row.address, "payout evidence poststate")?;
        parse_pubkey(&row.owner, "payout evidence poststate owner")?;
        if !addresses.insert(row.address.as_str()) {
            return Err(Error::new("payout evidence repeats a poststate account"));
        }
    }
    let row = evidence
        .poststates
        .iter()
        .find(|row| row.address == position.to_string())
        .ok_or_else(|| Error::new("payout evidence omitted its Direct Position poststate"))?;
    if row.owner != owner.to_string()
        || row.executable
        || row.lamports == 0
        || row.data_len != post.len()
        || row.data_sha256 != sha256(post)
    {
        return Err(Error::new(
            "payout evidence substituted its authenticated Position owner or postdata",
        ));
    }
    Ok(())
}

fn adapt_retirement(
    sources: &[RawSourceV1],
) -> Result<(
    Vec<SourceRefV1>,
    Vec<PendingEventV1>,
    BTreeMap<String, (String, String)>,
)> {
    let source = one_source(sources, "completion")?;
    let source_ref = source_ref(
        source,
        AGGREGATE_RETIREMENT_COMPLETION_SCHEMA_V1,
        "schema",
        "/status",
        &Value::String("finalized".into()),
    )?;
    let completion: AggregateRetirementConservationReceiptV1 =
        exact_deserialize(&source.bytes, "aggregate retirement completion")?;
    authenticate_aggregate_retirement_conservation_receipt_v1(&completion)?;
    if completion.payer == completion.refund_wallet {
        return Err(Error::new(
            "complete-life retirement requires a fee payer distinct from the creation-fixed refund beneficiary",
        ));
    }
    let (pending, kinds) = project_aggregate_retirement_completion(
        &completion,
        &source.relative,
        &sha256(&source.bytes),
    )?;
    Ok((vec![source_ref], pending, kinds))
}

fn project_aggregate_retirement_completion(
    completion: &AggregateRetirementConservationReceiptV1,
    source_path: &str,
    source_sha256: &str,
) -> Result<(Vec<PendingEventV1>, BTreeMap<String, (String, String)>)> {
    if completion.journals.len() != AggregateRetirementOperationV1::ORDERED.len()
        || completion
            .journals
            .iter()
            .map(|journal| journal.operation)
            .ne(AggregateRetirementOperationV1::ORDERED)
    {
        return Err(Error::new(
            "aggregate retirement completion did not contain exact ordered prepare, close-vault, close-replay, and finish rows",
        ));
    }
    let mut kinds = BTreeMap::from([
        (
            completion.market.clone(),
            ("protocol".into(), "retirement-market".into()),
        ),
        (
            completion.checkpoint.clone(),
            ("protocol".into(), "retirement-checkpoint".into()),
        ),
        (
            completion.rent_credit.clone(),
            ("protocol".into(), "retirement-rent-credit".into()),
        ),
        (
            completion.refund_wallet.clone(),
            ("wallet".into(), "retirement-refund-wallet".into()),
        ),
    ]);
    kinds.insert(
        completion.payer.clone(),
        ("wallet".into(), "retirement-fee-payer".into()),
    );
    let pending = completion
        .journals
        .iter()
        .map(|journal| PendingEventV1 {
            operation: journal.operation.label().into(),
            signature: journal.signature.clone(),
            expected_slot: journal.finalized_slot,
            expected_fee: journal.fee_lamports,
            expected_compute: Some(journal.compute_units_consumed),
            source_path: source_path.into(),
            source_sha256: source_sha256.into(),
            direct: None,
            positions: None,
            fee_settlement: None,
            forbidden_fee_payers: Vec::new(),
            required_transaction_accounts: Vec::new(),
            direct_fee_receipt: None,
            expected_return_data: None,
            expected_fee_payer: None,
            position: None,
            certificate: None,
            payout: None,
            retirement: Some({
                let mut value = json!({
                    "stage": journal.operation.completion_name(),
                    "_deriveFinalizedLamports": true,
                });
                if journal.operation == AggregateRetirementOperationV1::Finish {
                    value.as_object_mut().expect("retirement object").insert(
                        "_conservation".into(),
                        json!({
                            "refundBeneficiary": completion.refund_wallet,
                            "payer": completion.payer,
                            "classifiedLamports": {
                                "market": completion.classified_lamports.market.to_string(),
                                "rentCredit": completion.classified_lamports.rent_credit.to_string(),
                                "claimsRefund": completion.classified_lamports.claims_refund.to_string(),
                                "custodyReplay": completion.classified_lamports.custody_replay.to_string(),
                                "hoardVaultRent": completion.classified_lamports.hoard_vault.to_string(),
                                "expectedRefundDelta": completion.classified_lamports.expected_refund_delta.to_string(),
                                "refundWalletBefore": completion.classified_lamports.refund_wallet_before.to_string(),
                            },
                            "totalTransactionFeesLamports": completion.total_transaction_fees_lamports.to_string(),
                            "terminalRefundWalletLamports": completion.terminal_refund_wallet_lamports.to_string(),
                            "beneficiaryClass": "creation-fixed-refund-wallet",
                            "capitalizationClass": "historical-account-lamports-not-future-revenue-or-hoard-principal",
                        }),
                    );
                }
                value
            }),
        })
        .collect();
    Ok((pending, kinds))
}

fn one_source<'a>(sources: &'a [RawSourceV1], role: &str) -> Result<&'a RawSourceV1> {
    let matches = sources
        .iter()
        .filter(|source| source.role == role)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(Error::new(format!(
            "stage requires exactly one {role} source"
        )));
    }
    Ok(matches[0])
}

fn reopen_event(rpc: &mut Rpc, stage: &str, pending: PendingEventV1) -> Result<RpcEventV1> {
    Signature::from_str(&pending.signature)
        .map_err(|error| Error::new(format!("activity signature: {error}")))?;
    let result = rpc.call(
        "getTransaction",
        &json!([pending.signature, {
            "commitment": "finalized",
            "encoding": "json",
            "maxSupportedTransactionVersion": 0,
        }]),
    )?;
    let root = object(Some(&result), "finalized transaction")?;
    let slot = u64_field(root, "slot", "finalized transaction")?;
    let meta = object(root.get("meta"), "finalized transaction meta")?;
    if meta.get("err") != Some(&Value::Null) {
        return Err(Error::new("activity source signature is absent or failed"));
    }
    let fee = u64_field(meta, "fee", "finalized transaction meta")?;
    ExpectedClusterV1::OwnedLoopback
        .authenticate_finalized_fee(fee, "owned-loopback activity transaction")?;
    let compute = meta
        .get("computeUnitsConsumed")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized activity transaction omitted compute units"))?;
    if slot != pending.expected_slot
        || fee != pending.expected_fee
        || pending
            .expected_compute
            .is_some_and(|expected| expected != compute)
    {
        return Err(Error::new(
            "activity source slot, fee, or compute units differ from finalized history",
        ));
    }
    let transaction = object(root.get("transaction"), "finalized transaction body")?;
    let signatures = array(transaction.get("signatures"), "transaction signatures")?;
    if signatures.first().and_then(Value::as_str) != Some(pending.signature.as_str()) {
        return Err(Error::new(
            "getTransaction did not bind the requested first signature",
        ));
    }
    let keys = transaction_account_keys(transaction, meta)?;
    if pending.required_transaction_accounts.len()
        != pending
            .required_transaction_accounts
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
        || pending
            .required_transaction_accounts
            .iter()
            .any(|address| !keys.contains(address))
    {
        return Err(Error::new(
            "finalized Direct fee completion omitted or aliased an authenticated route account",
        ));
    }
    if let Some(expected) = pending.direct_fee_receipt.as_ref() {
        authenticate_direct_fee_return_data(meta, expected)?;
    }
    if let Some(expected) = pending.expected_return_data.as_ref() {
        authenticate_expected_return_data(meta, expected)?;
    }
    if pending.direct_fee_receipt.is_some() {
        let message = object(
            transaction.get("message"),
            "permissionless transaction message",
        )?;
        let header = object(message.get("header"), "permissionless transaction header")?;
        let required_signatures = u64_field(
            header,
            "numRequiredSignatures",
            "permissionless transaction header",
        )?;
        if required_signatures != 1 || signatures.len() != 1 {
            return Err(Error::new(
                "permissionless Direct fee completion carried a protocol-party signer",
            ));
        }
    }
    let pre = u64_array(meta.get("preBalances"), "preBalances")?;
    let post = u64_array(meta.get("postBalances"), "postBalances")?;
    if pre.len() != keys.len() || post.len() != keys.len() {
        return Err(Error::new(
            "finalized transaction balance vectors differ from account keys",
        ));
    }
    let mut changed = BTreeSet::new();
    let mut lamports = Vec::new();
    for (index, address) in keys.iter().enumerate() {
        let delta = i128::from(post[index]) - i128::from(pre[index]);
        if delta != 0 {
            changed.insert(address.clone());
            lamports.push(DeltaV1 {
                account: address.clone(),
                lamports: Some(delta.to_string()),
                atoms: None,
            });
        }
    }
    let pre_tokens = token_balances(meta.get("preTokenBalances"), &keys, "preTokenBalances")?;
    let post_tokens = token_balances(meta.get("postTokenBalances"), &keys, "postTokenBalances")?;
    let mut token_identities = BTreeMap::new();
    let mut token_deltas = Vec::new();
    for address in pre_tokens
        .keys()
        .chain(post_tokens.keys())
        .collect::<BTreeSet<_>>()
    {
        let before = pre_tokens.get(address.as_str());
        let after = post_tokens.get(address.as_str());
        let identity = after
            .or(before)
            .ok_or_else(|| Error::new("token identity disappeared"))?;
        if before.is_some_and(|value| value.0 != identity.0)
            || after.is_some_and(|value| value.0 != identity.0)
        {
            return Err(Error::new(
                "finalized token balance substitutes mint, authority, or program owner",
            ));
        }
        token_identities.insert(address.to_string(), identity.0.clone());
        let before_amount = before.map_or(0, |value| value.1);
        let after_amount = after.map_or(0, |value| value.1);
        let delta = i128::from(after_amount) - i128::from(before_amount);
        if delta != 0 {
            changed.insert(address.to_string());
            token_deltas.push(DeltaV1 {
                account: address.to_string(),
                lamports: None,
                atoms: Some(delta.to_string()),
            });
        }
    }
    let fee_payer = keys
        .first()
        .cloned()
        .ok_or_else(|| Error::new("finalized transaction has no fee payer"))?;
    if pending
        .expected_fee_payer
        .as_deref()
        .is_some_and(|expected| expected != fee_payer)
    {
        return Err(Error::new(
            "activity transaction fee payer differs from its semantic owner",
        ));
    }
    if pending
        .forbidden_fee_payers
        .iter()
        .any(|address| address == &fee_payer)
    {
        return Err(Error::new(
            "activity transaction charged a forbidden protocol party or holder",
        ));
    }
    changed.insert(fee_payer.clone());
    Ok(RpcEventV1 {
        event: EventV1 {
            id: String::new(),
            kind: stage.into(),
            operation: pending.operation,
            predecessor: None,
            signature: pending.signature,
            slot: slot.to_string(),
            fee_payer,
            fee_lamports: fee.to_string(),
            compute_units_consumed: compute.to_string(),
            lamport_deltas: lamports,
            token_deltas,
            source_path: pending.source_path,
            source_sha256: pending.source_sha256,
            direct: pending.direct,
            positions: pending.positions,
            fee_settlement: pending.fee_settlement,
            position: pending.position,
            certificate: pending.certificate,
            payout: pending.payout,
            retirement: pending.retirement,
        },
        transaction: result,
        token_identities,
        changed_addresses: changed,
    })
}

fn authenticate_direct_fee_return_data(
    meta: &Map<String, Value>,
    expected: &DirectFeeReceiptExpectationV1,
) -> Result<()> {
    let return_data = object(
        meta.get("returnData").filter(|value| !value.is_null()),
        "Direct fee-settlement returnData",
    )?;
    let producer = pubkey_field(return_data, "programId", "Direct fee-settlement returnData")?;
    let tuple = array(
        return_data.get("data"),
        "Direct fee-settlement returnData body",
    )?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(Error::new(
            "Direct fee-settlement returnData body was not exact [body, base64]",
        ));
    }
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("Direct fee-settlement returnData omitted its body"))?;
    let body = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("Direct fee-settlement returnData base64: {error}")))?;
    if BASE64.encode(&body) != encoded {
        return Err(Error::new(
            "Direct fee-settlement returnData base64 was noncanonical",
        ));
    }
    let receipt = DirectFeeSettlementReceiptV1::decode(&body)
        .map_err(|error| Error::new(format!("Direct fee-settlement return receipt: {error:?}")))?;
    let expected_pubkey = |value: &str, label: &str| -> Result<[u8; 32]> {
        Ok(Pubkey::from_str(value)
            .map_err(|error| Error::new(format!("{label}: {error}")))?
            .to_bytes())
    };
    if producer != expected.producer
        || receipt.market != expected_pubkey(&expected.market, "fee receipt Market")?
        || receipt.maker != expected_pubkey(&expected.maker, "fee receipt maker")?
        || receipt.maker_root
            != expected_pubkey(&expected.maker_replay, "fee receipt maker replay")?
        || receipt.fee_source != expected_pubkey(&expected.fee_source, "fee receipt source")?
        || receipt.fee_destination
            != expected_pubkey(&expected.fee_destination, "fee receipt destination")?
        || receipt.fee_recipient
            != expected_pubkey(&expected.fee_recipient, "fee receipt recipient")?
        || receipt.settled_amount != expected.settled_amount
        || receipt.expected_revision != expected.expected_revision
        || receipt.resulting_revision != expected.resulting_revision
    {
        return Err(Error::new(
            "finalized Direct fee receipt differs from its exact authenticated obligation",
        ));
    }
    Ok(())
}

fn authenticate_expected_return_data(
    meta: &Map<String, Value>,
    expected: &ExpectedReturnDataV1,
) -> Result<()> {
    let return_data = object(
        meta.get("returnData").filter(|value| !value.is_null()),
        "activity transaction returnData",
    )?;
    let producer = pubkey_field(return_data, "programId", "activity transaction returnData")?;
    let tuple = array(
        return_data.get("data"),
        "activity transaction returnData body",
    )?;
    if producer != expected.producer
        || tuple.len() != 2
        || tuple.get(1).and_then(Value::as_str) != Some("base64")
    {
        return Err(Error::new(
            "activity transaction returnData changed producer or encoding",
        ));
    }
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("activity transaction returnData omitted its body"))?;
    let body = decode_canonical_base64(encoded, "activity transaction returnData")?;
    if body != expected.body {
        return Err(Error::new(
            "activity transaction returnData differs from its semantic owner",
        ));
    }
    Ok(())
}

fn transaction_account_keys(
    transaction: &Map<String, Value>,
    meta: &Map<String, Value>,
) -> Result<Vec<String>> {
    let message = object(transaction.get("message"), "transaction message")?;
    let mut keys = Vec::new();
    for raw in array(message.get("accountKeys"), "transaction account keys")? {
        let value = raw
            .as_str()
            .or_else(|| raw.get("pubkey").and_then(Value::as_str))
            .ok_or_else(|| Error::new("transaction account key is not a pubkey"))?;
        parse_pubkey(value, "transaction account key")?;
        keys.push(value.into());
    }
    if let Some(loaded) = meta.get("loadedAddresses").filter(|value| !value.is_null()) {
        let loaded = object(Some(loaded), "loaded addresses")?;
        for field in ["writable", "readonly"] {
            for raw in array(loaded.get(field), "loaded address list")? {
                let value = raw
                    .as_str()
                    .ok_or_else(|| Error::new("loaded address is not text"))?;
                parse_pubkey(value, "loaded address")?;
                keys.push(value.into());
            }
        }
    }
    if keys.len() != keys.iter().collect::<BTreeSet<_>>().len() {
        return Err(Error::new("transaction account vector aliases an address"));
    }
    Ok(keys)
}

type TokenBalanceV1 = (TokenIdentityV1, u64);

fn token_balances(
    raw: Option<&Value>,
    keys: &[String],
    label: &str,
) -> Result<BTreeMap<String, TokenBalanceV1>> {
    let Some(raw) = raw else {
        return Ok(BTreeMap::new());
    };
    let mut out = BTreeMap::new();
    for entry in array(Some(raw), label)? {
        let entry = object(Some(entry), label)?;
        let index = usize::try_from(u64_field(entry, "accountIndex", label)?)
            .map_err(|_| Error::new("token account index exceeds usize"))?;
        let address = keys
            .get(index)
            .ok_or_else(|| Error::new("token account index is outside account keys"))?;
        let mint = pubkey_field(entry, "mint", label)?;
        let authority = pubkey_field(entry, "owner", label)?;
        let program_owner = pubkey_field(entry, "programId", label)?;
        let ui = object(entry.get("uiTokenAmount"), "uiTokenAmount")?;
        let amount = canonical_decimal(
            string_field(ui, "amount", "uiTokenAmount")?,
            "token atoms",
            false,
        )?;
        let row = (
            TokenIdentityV1 {
                mint,
                authority,
                program_owner,
            },
            amount,
        );
        if out.insert(address.clone(), row).is_some() {
            return Err(Error::new("token balance vector repeats an account"));
        }
    }
    Ok(out)
}

fn project_accounts(
    rpc: &mut Rpc,
    stage: &str,
    mut rpc_events: Vec<RpcEventV1>,
    kind_overrides: BTreeMap<String, (String, String)>,
) -> Result<(Vec<AccountV1>, Vec<FinalAccountV1>, Vec<EventV1>)> {
    if rpc_events.is_empty() || rpc_events.len() > MAX_EVENTS {
        return Err(Error::new(
            "activity stage has no bounded finalized mutation set",
        ));
    }
    let mut addresses = kind_overrides.keys().cloned().collect::<BTreeSet<_>>();
    let mut token_identities = BTreeMap::new();
    let mut prior_slot = 0;
    for observed in &rpc_events {
        let slot = canonical_decimal(&observed.event.slot, "event slot", false)?;
        if slot < prior_slot {
            return Err(Error::new("activity stage transaction slots regress"));
        }
        prior_slot = slot;
        addresses.extend(observed.changed_addresses.iter().cloned());
        for (address, identity) in &observed.token_identities {
            if token_identities
                .insert(address.clone(), identity.clone())
                .is_some_and(|prior: TokenIdentityV1| {
                    prior.mint != identity.mint
                        || prior.authority != identity.authority
                        || prior.program_owner != identity.program_owner
                })
            {
                return Err(Error::new(
                    "activity token identity changed across stage history",
                ));
            }
        }
    }
    if addresses.len() > MAX_ACCOUNTS {
        return Err(Error::new("activity stage account set exceeds 512"));
    }
    let (_, current) = finalized_multiple_accounts(rpc, &addresses)?;
    for event in &mut rpc_events {
        finalize_semantic_facts(&mut event.event, &current)?;
    }
    if stage == "payout" {
        authenticate_payout_terminal_positions(&rpc_events, &current)?;
    }
    let retirement_closed = if stage == "retirement" {
        exact_retirement_closed_accounts(&rpc_events, &current)?
    } else {
        BTreeSet::new()
    };
    let fee_payers = rpc_events
        .iter()
        .map(|row| row.event.fee_payer.clone())
        .collect::<BTreeSet<_>>();
    let mut accounts = Vec::with_capacity(addresses.len());
    let mut critical = BTreeSet::new();
    for address in addresses {
        let override_row = kind_overrides.get(&address);
        let kind = override_row
            .map(|row| row.0.as_str())
            .or_else(|| token_identities.contains_key(&address).then_some("token"))
            .or_else(|| fee_payers.contains(&address).then_some("wallet"))
            .unwrap_or("protocol");
        let role = match kind {
            "wallet" => "wallet",
            "token" => "collateral-token-account",
            "position" => "claims-position",
            "certificate" => "resolution-certificate",
            _ => "protocol-account",
        }
        .to_owned();
        let mut account = AccountV1 {
            r#ref: address.clone(),
            address: address.clone(),
            kind: kind.into(),
            role,
            mint: None,
            asset_class: None,
            authority: None,
            program_owner: None,
        };
        if kind == "token" {
            let identity = token_identities
                .get(&address)
                .cloned()
                .or_else(|| {
                    current
                        .get(&address)
                        .and_then(|value| value.as_ref())
                        .and_then(decode_live_token_identity)
                })
                .ok_or_else(|| {
                    Error::new(format!("token account {address} has no exact identity"))
                })?;
            account.mint = Some(identity.mint);
            account.asset_class = Some("collateral".into());
            account.authority = Some(identity.authority);
            account.program_owner = Some(identity.program_owner);
        }
        if matches!(kind, "token" | "position" | "certificate") {
            critical.insert(address.clone());
        }
        accounts.push(account);
    }
    let by_address = accounts
        .iter()
        .map(|row| (row.address.clone(), row))
        .collect::<BTreeMap<_, _>>();
    let final_owners = match stage {
        "founding" | "participant" => BTreeSet::new(),
        "retirement" => retirement_closed
            .into_iter()
            .chain(kind_overrides.iter().filter_map(|(address, (_, role))| {
                (role == "retirement-refund-wallet").then(|| address.clone())
            }))
            .collect(),
        "resolution" => critical
            .iter()
            .filter(|address| {
                by_address
                    .get(*address)
                    .is_some_and(|row| row.kind == "certificate")
            })
            .cloned()
            .collect(),
        "payout" => critical,
        _ => BTreeSet::new(),
    };
    let mut final_accounts = Vec::with_capacity(final_owners.len());
    for address in final_owners {
        let account = by_address
            .get(&address)
            .ok_or_else(|| Error::new("critical account disappeared from projection"))?;
        let Some(value) = current.get(&address).and_then(|value| value.as_ref()) else {
            final_accounts.push(FinalAccountV1 {
                account: address,
                closed: true,
                owner: None,
                lamports: None,
                data_sha256: None,
                mint: None,
                authority: None,
                amount_atoms: None,
            });
            continue;
        };
        let row = rpc_account(value, "final activity account")?;
        let data = account_data(row, "final activity account")?;
        let mut final_row = FinalAccountV1 {
            account: address,
            closed: false,
            owner: Some(pubkey_field(row, "owner", "final activity account")?),
            lamports: Some(u64_field(row, "lamports", "final activity account")?.to_string()),
            data_sha256: Some(sha256(&data)),
            mint: None,
            authority: None,
            amount_atoms: None,
        };
        if account.kind == "token" {
            let (mint, authority, amount) = decode_token_account(&data)?;
            if account.mint.as_deref() != Some(&mint)
                || account.authority.as_deref() != Some(&authority)
                || account.program_owner != final_row.owner
            {
                return Err(Error::new(
                    "final token identity differs from activity account",
                ));
            }
            final_row.mint = Some(mint);
            final_row.authority = Some(authority);
            final_row.amount_atoms = Some(amount.to_string());
        }
        final_accounts.push(final_row);
    }
    Ok((
        accounts,
        final_accounts,
        rpc_events.into_iter().map(|row| row.event).collect(),
    ))
}

fn authenticate_payout_terminal_positions(
    events: &[RpcEventV1],
    current: &BTreeMap<String, Option<Value>>,
) -> Result<()> {
    let mut terminal = BTreeMap::new();
    for event in events {
        let position = object(event.event.position.as_ref(), "payout Position transition")?;
        let address = pubkey_field(position, "account", "payout Position transition")?;
        let owner = pubkey_field(position, "owner", "payout Position transition")?;
        let post = decode_canonical_base64(
            string_field(position, "postDataBase64", "payout Position transition")?,
            "payout Position transition poststate",
        )?;
        terminal.insert(address, (owner, post));
    }
    if terminal.is_empty() {
        return Err(Error::new("payout stage omitted its Position transitions"));
    }
    for (address, (owner, expected)) in terminal {
        match current.get(&address) {
            Some(Some(value)) => {
                let row = rpc_account(value, "terminal payout Position")?;
                if pubkey_field(row, "owner", "terminal payout Position")? != owner
                    || account_data(row, "terminal payout Position")? != expected
                {
                    return Err(Error::new(
                        "live payout Position differs from the history-replayed terminal poststate",
                    ));
                }
            }
            Some(None) => {}
            None => {
                return Err(Error::new(
                    "payout Position closure is outside the finalized account set",
                ));
            }
        }
    }
    Ok(())
}

fn exact_retirement_closed_accounts(
    events: &[RpcEventV1],
    current: &BTreeMap<String, Option<Value>>,
) -> Result<BTreeSet<String>> {
    if events.len() != AggregateRetirementOperationV1::ORDERED.len() {
        return Err(Error::new(
            "retirement activity requires four AggregateRetirement events",
        ));
    }
    let mut closed = BTreeSet::new();
    for (observed, operation) in events.iter().zip(AggregateRetirementOperationV1::ORDERED) {
        let retirement = object(
            observed.event.retirement.as_ref(),
            "finalized retirement projection",
        )?;
        if observed.event.operation != operation.label()
            || retirement.get("stage").and_then(Value::as_str) != Some(operation.completion_name())
        {
            return Err(Error::new(
                "retirement activity changed exact prepare, close-vault, close-replay, finish order",
            ));
        }
        for candidate in array(
            retirement.get("closedAccounts"),
            "retirement closed accounts",
        )? {
            let address = candidate
                .as_str()
                .ok_or_else(|| Error::new("retirement closed account is not text"))?;
            Pubkey::from_str(address)
                .map_err(|error| Error::new(format!("retirement closed account: {error}")))?;
            if !closed.insert(address.to_owned()) {
                return Err(Error::new(
                    "retirement history closes the same account more than once",
                ));
            }
            match current.get(address) {
                Some(None) => {}
                Some(Some(_)) => {
                    return Err(Error::new(
                        "retirement closed-account fact is live at finalized context",
                    ));
                }
                None => {
                    return Err(Error::new(
                        "retirement closed-account fact is outside the finalized account set",
                    ));
                }
            }
        }
    }
    Ok(closed)
}

fn finalize_semantic_facts(
    event: &mut EventV1,
    current: &BTreeMap<String, Option<Value>>,
) -> Result<()> {
    if let Some(raw) = event.certificate.take() {
        let value = object(Some(&raw), "pending certificate")?;
        if value.get("_deriveFinalizedAccount") != Some(&Value::Bool(true)) {
            return Err(Error::new(
                "resolution certificate projection is not derived",
            ));
        }
        let address = pubkey_field(value, "account", "pending certificate")?;
        let market = pubkey_field(value, "market", "pending certificate")?;
        let account = current
            .get(&address)
            .and_then(|value| value.as_ref())
            .ok_or_else(|| Error::new("terminal Resolution certificate is absent"))?;
        let row = rpc_account(account, "resolution certificate")?;
        event.certificate = Some(json!({
            "account": address,
            "owner": pubkey_field(row, "owner", "resolution certificate")?,
            "dataBase64": BASE64.encode(account_data(row, "resolution certificate")?),
            "market": market,
        }));
    }
    if let Some(raw) = event.payout.take() {
        let value = object(Some(&raw), "pending payout")?;
        let position = pubkey_field(value, "_positionAddress", "pending payout")?;
        let claim_index = usize::try_from(u64_field(value, "_claimIndex", "pending payout")?)
            .map_err(|_| Error::new("payout claim index exceeds usize"))?;
        let quantity = canonical_decimal(
            string_field(value, "_quantity", "pending payout")?,
            "payout quantity",
            true,
        )?;
        let principal = canonical_decimal(
            string_field(value, "principalAtoms", "pending payout")?,
            "payout principal",
            true,
        )?;
        let pre = decode_canonical_base64(
            string_field(value, "_preDataBase64", "pending payout")?,
            "pending payout Position prestate",
        )?;
        let post = decode_canonical_base64(
            string_field(value, "_postDataBase64", "pending payout")?,
            "pending payout Position poststate",
        )?;
        let (expected_post, burns) = apply_position_payout(&pre, claim_index, quantity)?;
        if expected_post != post {
            return Err(Error::new(
                "payout Position history did not replay to its evidence-owned poststate",
            ));
        }
        event.position = Some(json!({
            "account": position,
            "owner": pubkey_field(value, "_positionOwner", "pending payout")?,
            "preDataBase64": BASE64.encode(&pre),
            "postDataBase64": BASE64.encode(&post),
        }));
        event.payout = Some(json!({
            "hoardToken": pubkey_field(value, "hoardToken", "pending payout")?,
            "recipientToken": pubkey_field(value, "recipientToken", "pending payout")?,
            "position": position,
            "principalAtoms": principal.to_string(),
            "claimsBurnedAtoms": burns,
            "mint": pubkey_field(value, "mint", "pending payout")?,
            "holder": pubkey_field(value, "holder", "pending payout")?,
            "holderChargeClass": string_field(value, "holderChargeClass", "pending payout")?,
        }));
    }
    if let Some(raw) = event.retirement.take() {
        let value = object(Some(&raw), "pending retirement")?;
        if value.get("_deriveFinalizedLamports") != Some(&Value::Bool(true)) {
            return Err(Error::new(
                "retirement projection was not derived from finalized transaction lamports",
            ));
        }
        let mut closed = Vec::new();
        let mut refunds = Vec::new();
        for delta in &event.lamport_deltas {
            let amount = canonical_signed_decimal(
                delta
                    .lamports
                    .as_deref()
                    .ok_or_else(|| Error::new("retirement lamport delta omitted lamports"))?,
                "retirement finalized lamport delta",
            )?;
            if amount < 0 && current.get(&delta.account).is_some_and(Option::is_none) {
                closed.push(delta.account.clone());
            } else if amount > 0 {
                refunds.push(json!({
                    "account": delta.account.clone(),
                    "lamports": amount.to_string(),
                }));
            }
        }
        let mut retirement = json!({
            "stage": string_field(value, "stage", "pending retirement")?,
            "closedAccounts": closed,
            "refundLamports": refunds,
        });
        if let Some(conservation) = value.get("_conservation") {
            retirement
                .as_object_mut()
                .expect("retirement projection")
                .insert("conservation".into(), conservation.clone());
        }
        event.retirement = Some(retirement);
    }
    Ok(())
}

fn apply_position_payout(
    pre: &[u8],
    claim_index: usize,
    quantity: u64,
) -> Result<(Vec<u8>, Vec<String>)> {
    if quantity == 0 || pre.len() < 128 || pre.get(0..8) != Some(b"DCLLBP02") {
        return Err(Error::new(
            "payout Position is not exact LiabilityBasisPositionV2 or quantity is zero",
        ));
    }
    let count = usize::try_from(u32::from_le_bytes(
        pre.get(12..16)
            .ok_or_else(|| Error::new("Position omitted claim count"))?
            .try_into()
            .map_err(|_| Error::new("Position claim count width"))?,
    ))
    .map_err(|_| Error::new("Position claim count exceeds usize"))?;
    if count < 2 || claim_index >= count || pre.len() != 128 + 8 * count {
        return Err(Error::new(
            "payout Position geometry or claim index is invalid",
        ));
    }
    let mut post = pre.to_vec();
    let revision = u64::from_le_bytes(post[16..24].try_into().unwrap());
    let next_revision = revision
        .checked_add(1)
        .ok_or_else(|| Error::new("payout Position revision overflows"))?;
    post[16..24].copy_from_slice(&next_revision.to_le_bytes());
    let offset = 128 + 8 * claim_index;
    let pre_amount = u64::from_le_bytes(post[offset..offset + 8].try_into().unwrap());
    let post_amount = pre_amount
        .checked_sub(quantity)
        .ok_or_else(|| Error::new("payout Position claim balance is insufficient"))?;
    post[offset..offset + 8].copy_from_slice(&post_amount.to_le_bytes());
    let mut burns = vec!["0".to_owned(); count];
    burns[claim_index] = quantity.to_string();
    Ok((post, burns))
}

fn reconstruct_position_prestate(
    post: &[u8],
    claim_index: usize,
    quantity: u64,
) -> Result<(Vec<u8>, Vec<String>)> {
    if post.len() < 128 || post.get(0..8) != Some(b"DCLLBP02") {
        return Err(Error::new(
            "payout Position is not exact LiabilityBasisPositionV2",
        ));
    }
    let count = usize::try_from(u32::from_le_bytes(
        post.get(12..16)
            .ok_or_else(|| Error::new("Position omitted claim count"))?
            .try_into()
            .map_err(|_| Error::new("Position claim count width"))?,
    ))
    .map_err(|_| Error::new("Position claim count exceeds usize"))?;
    if count < 2 || claim_index >= count || post.len() != 128 + 8 * count {
        return Err(Error::new(
            "payout Position geometry or claim index is invalid",
        ));
    }
    let mut pre = post.to_vec();
    let revision = u64::from_le_bytes(pre[16..24].try_into().unwrap());
    let prior_revision = revision
        .checked_sub(1)
        .ok_or_else(|| Error::new("payout Position revision did not advance"))?;
    pre[16..24].copy_from_slice(&prior_revision.to_le_bytes());
    let offset = 128 + 8 * claim_index;
    let post_amount = u64::from_le_bytes(pre[offset..offset + 8].try_into().unwrap());
    let pre_amount = post_amount
        .checked_add(quantity)
        .ok_or_else(|| Error::new("payout Position prestate overflows"))?;
    pre[offset..offset + 8].copy_from_slice(&pre_amount.to_le_bytes());
    let mut burns = vec!["0".to_owned(); count];
    burns[claim_index] = quantity.to_string();
    Ok((pre, burns))
}

pub(crate) fn run_manifest(arguments: ManifestArgumentsV1) -> Result<Value> {
    let origin = owned_loopback_origin(&arguments.rpc_url)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let plan = authenticate_plan(&arguments.plan)?;
    let (_, manifest) = derive_manifest(&mut rpc, &plan, &arguments.stage_journal_descriptors)?;
    let value = serde_json::to_value(manifest)?;
    write_new_json(&arguments.output, &value)?;
    Ok(value)
}

pub(crate) fn run_capture(arguments: CaptureArgumentsV1) -> Result<Value> {
    let origin = owned_loopback_origin(&arguments.rpc_url)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let plan = authenticate_plan(&arguments.plan)?;
    let (_, expected_manifest) =
        derive_manifest(&mut rpc, &plan, &arguments.stage_journal_descriptors)?;
    let manifest_path = canonical_regular(&arguments.activity_manifest, "activity manifest")?;
    let manifest: ManifestV1 = exact_deserialize(
        &bounded_read(&manifest_path, "activity manifest")?,
        "activity manifest",
    )?;
    if manifest != expected_manifest {
        return Err(Error::new(
            "activity manifest differs from the six authenticated stage completions",
        ));
    }
    let genesis = manifest_genesis(&manifest)?;
    let mut account_addresses = manifest
        .accounts
        .iter()
        .map(|row| row.address.clone())
        .collect::<BTreeSet<_>>();
    account_addresses.extend(
        manifest
            .final_accounts
            .iter()
            .map(|row| row.account.clone()),
    );
    let loader_addresses = exact_loader_addresses(&plan)?;
    account_addresses.extend(loader_addresses.iter().cloned());
    if account_addresses.len() > MAX_ACCOUNTS {
        return Err(Error::new(
            "finalized capture exceeds the 512-account bound",
        ));
    }
    let (finalized_slot, values) = finalized_multiple_accounts(&mut rpc, &account_addresses)?;
    if finalized_slot == 0 {
        return Err(Error::new("finalized capture boundary is zero"));
    }
    refuse_unexpected_loader_accounts(&values, &loader_addresses)?;
    let closed = manifest
        .final_accounts
        .iter()
        .filter(|row| row.closed)
        .map(|row| row.account.clone())
        .chain(manifest.events.iter().flat_map(|event| {
            event
                .retirement
                .as_ref()
                .and_then(|value| value.get("closedAccounts"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
        }))
        .collect::<BTreeSet<_>>();
    let mut captured_accounts = Map::new();
    for address in &account_addresses {
        captured_accounts.insert(
            address.clone(),
            captured_account_row(
                &values,
                address,
                closed.contains(address),
                loader_addresses.contains(address),
                finalized_slot,
            )?,
        );
    }
    let signatures = manifest
        .events
        .iter()
        .map(|event| event.signature.clone())
        .collect::<BTreeSet<_>>();
    if signatures.len() != manifest.events.len() {
        return Err(Error::new(
            "activity manifest repeats a transaction signature",
        ));
    }
    let mut transactions = Map::new();
    for signature in signatures {
        let transaction = rpc.call(
            "getTransaction",
            &json!([signature, {
                "commitment": "finalized",
                "encoding": "json",
                "maxSupportedTransactionVersion": 0,
            }]),
        )?;
        validate_captured_transaction(&transaction, &signature, finalized_slot)?;
        transactions.insert(signature, transaction);
    }
    let repeated_genesis = owned_genesis(&mut rpc)?;
    if repeated_genesis != genesis {
        return Err(Error::new(
            "owned-loopback genesis changed across finalized capture",
        ));
    }
    let capture = json!({
        "schema": CAPTURE_SCHEMA_V1,
        "genesisHash": genesis,
        "commitment": "finalized",
        "finalizedSlot": finalized_slot.to_string(),
        "transactions": transactions,
        "accounts": captured_accounts,
    });
    let encoded = canonical_json(&capture)?;
    if encoded.len() as u64 > MAX_JSON_BYTES {
        return Err(Error::new("finalized activity capture exceeds 32 MiB"));
    }
    write_new_bytes(&arguments.output, &encoded)?;
    Ok(capture)
}

fn captured_account_row(
    values: &BTreeMap<String, Option<Value>>,
    address: &str,
    is_closed: bool,
    is_loader: bool,
    finalized_slot: u64,
) -> Result<Value> {
    let value = values
        .get(address)
        .ok_or_else(|| Error::new("getMultipleAccounts omitted one requested address"))?;
    if value.is_none() && (!is_closed || is_loader) {
        return Err(Error::new(format!(
            "finalized capture returned unexpected null for {address}"
        )));
    }
    Ok(json!({
        "contextSlot": finalized_slot.to_string(),
        "value": value,
    }))
}

fn refuse_unexpected_loader_accounts(
    values: &BTreeMap<String, Option<Value>>,
    exact_loader_addresses: &BTreeSet<String>,
) -> Result<()> {
    let loader = bpf_loader_upgradeable::ID.to_string();
    for (address, value) in values {
        if !exact_loader_addresses.contains(address)
            && value
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|account| account.get("owner"))
                .and_then(Value::as_str)
                == Some(loader.as_str())
        {
            return Err(Error::new(
                "finalized capture includes a Loader-v3 account outside the exact 18-account closure",
            ));
        }
    }
    Ok(())
}

fn validate_captured_transaction(
    transaction: &Value,
    expected_signature: &str,
    finalized_slot: u64,
) -> Result<()> {
    let root = object(Some(transaction), "captured finalized transaction")?;
    let slot = u64_field(root, "slot", "captured finalized transaction")?;
    let meta = object(root.get("meta"), "captured transaction meta")?;
    let body = object(root.get("transaction"), "captured transaction body")?;
    if slot > finalized_slot
        || meta.get("err") != Some(&Value::Null)
        || array(body.get("signatures"), "captured signatures")?
            .first()
            .and_then(Value::as_str)
            != Some(expected_signature)
    {
        return Err(Error::new(
            "captured transaction is null, failed, substituted, or newer than account boundary",
        ));
    }
    Ok(())
}

fn derive_manifest(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    descriptor_path: &Path,
) -> Result<(PathBuf, ManifestV1)> {
    let descriptor_path = canonical_regular(descriptor_path, "stage journal descriptors")?;
    let root = descriptor_path
        .parent()
        .ok_or_else(|| Error::new("stage journal descriptors have no evidence root"))?;
    let root = canonical_directory(root, "stage journal evidence root")?;
    let descriptors: JournalDescriptorDocumentV1 = exact_deserialize(
        &bounded_read(&descriptor_path, "stage journal descriptors")?,
        "stage journal descriptors",
    )?;
    if descriptors.schema != JOURNAL_DESCRIPTORS_SCHEMA_V1
        || descriptors.journals.is_empty()
        || descriptors.journals.len() > 160
    {
        return Err(Error::new(
            "stage journal descriptors are another schema or outside the 1..160 bound",
        ));
    }
    let genesis = owned_genesis(rpc)?;
    let mut seen_paths = BTreeSet::new();
    let mut stages = BTreeMap::new();
    for descriptor in descriptors.journals {
        canonical_relative(&descriptor.path, "journal descriptor path")?;
        if !seen_paths.insert(descriptor.path.clone()) {
            return Err(Error::new("stage journal descriptors repeat a path"));
        }
        let path = resolve_relative(&root, &descriptor.path, "journal descriptor source")?;
        let bytes = bounded_read(&path, "journal descriptor source")?;
        let value = parse_json_without_duplicate_keys_v1(&bytes)?;
        if value.get("schema").and_then(Value::as_str) != Some(descriptor.schema.as_str())
            || json_pointer(&value, &descriptor.completion_pointer, "journal descriptor")?
                != &Value::String("finalized".into())
        {
            return Err(Error::new(
                "journal descriptor source is substituted, provisional, or another schema",
            ));
        }
        if descriptor.schema == STAGE_SCHEMA_V1 {
            authenticate_normalized_stage_descriptor(&descriptor)?;
            let completion: StageCompletionV1 = serde_json::from_value(value.clone())?;
            authenticate_stage_completion(
                rpc,
                &root,
                &descriptor.semantic_role,
                &genesis,
                &completion,
            )?;
            if stages
                .insert(
                    descriptor.semantic_role,
                    (descriptor.path, sha256(&bytes), completion),
                )
                .is_some()
            {
                return Err(Error::new("activity descriptors repeat a stage wrapper"));
            }
        }
    }
    if stages.keys().map(String::as_str).collect::<BTreeSet<_>>() != STAGES.into_iter().collect() {
        return Err(Error::new(
            "activity descriptors omit the exact six semantic stage wrappers",
        ));
    }
    let checked = plan
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("activity plan omitted checked local mutable set"))?;
    if checked.roles.len() != 7 {
        return Err(Error::new(
            "activity plan omitted exact seven program roles",
        ));
    }
    let mut accounts_by_address: BTreeMap<String, AccountV1> = BTreeMap::new();
    let mut final_by_address: BTreeMap<String, FinalAccountV1> = BTreeMap::new();
    let mut events = Vec::new();
    let mut wrapper_set = Vec::new();
    for stage in STAGES {
        let (path, wrapper_sha, completion) = stages
            .get(stage)
            .ok_or_else(|| Error::new("activity stage wrapper disappeared"))?;
        wrapper_set.push(json!({"stage": stage, "sha256": wrapper_sha}));
        for account in &completion.accounts {
            merge_account_identity(&mut accounts_by_address, account)?;
        }
        for final_account in &completion.final_accounts {
            merge_final_account(&mut final_by_address, final_account)?;
        }
        for source_event in &completion.events {
            let mut event = source_event.clone();
            event.source_path = path.clone();
            event.source_sha256 = wrapper_sha.clone();
            events.push(event);
        }
    }
    if events.len() < STAGES.len() || events.len() > MAX_EVENTS {
        return Err(Error::new(
            "activity event chain is incomplete or exceeds 128",
        ));
    }
    let mut signatures = BTreeSet::new();
    let mut predecessor = None;
    let mut prior_slot = 0;
    for (index, event) in events.iter_mut().enumerate() {
        if !signatures.insert(event.signature.clone()) {
            return Err(Error::new("activity stage wrappers repeat a signature"));
        }
        let slot = canonical_decimal(&event.slot, "activity event slot", false)?;
        if slot < prior_slot {
            return Err(Error::new("activity event slots regress across stages"));
        }
        prior_slot = slot;
        event.id = format!("activity-{index:03}");
        event.predecessor = predecessor.clone();
        predecessor = Some(event.id.clone());
    }
    let accounts = accounts_by_address.into_values().collect::<Vec<_>>();
    let final_accounts = final_by_address.into_values().collect::<Vec<_>>();
    let critical = accounts
        .iter()
        .filter(|row| matches!(row.kind.as_str(), "token" | "position" | "certificate"))
        .map(|row| row.r#ref.clone())
        .collect::<BTreeSet<_>>();
    if !critical.is_subset(
        &final_accounts
            .iter()
            .map(|row| row.account.clone())
            .collect(),
    ) {
        return Err(Error::new(
            "joined stage wrappers omit a critical final account",
        ));
    }
    let source_set = events
        .iter()
        .map(|event| json!({"event": event.id, "sha256": event.source_sha256}))
        .collect::<Vec<_>>();
    let activity_digest = sha256(&canonical_json(&json!({
        "genesisHash": genesis,
        "wrappers": wrapper_set,
    }))?);
    Ok((
        root,
        ManifestV1 {
            schema: MANIFEST_SCHEMA_V1.into(),
            activity_id: format!("owned-loopback-{}", &activity_digest[..32]),
            cluster: json!({"kind": "owned-loopback", "genesisHash": genesis}),
            accounts,
            events,
            final_accounts,
            source_set_sha256: sha256(&canonical_json(&Value::Array(source_set))?),
        },
    ))
}

fn authenticate_normalized_stage_descriptor(descriptor: &JournalDescriptorV1) -> Result<()> {
    if descriptor.schema != STAGE_SCHEMA_V1
        || !STAGES.contains(&descriptor.semantic_role.as_str())
        || descriptor.completion_pointer != "/status"
    {
        return Err(Error::new(
            "normalized activity stage descriptor does not bind its exact top-level status owner",
        ));
    }
    Ok(())
}

fn authenticate_stage_completion(
    rpc: &mut Rpc,
    root: &Path,
    expected_stage: &str,
    expected_genesis: &str,
    completion: &StageCompletionV1,
) -> Result<()> {
    if completion.schema != STAGE_SCHEMA_V1
        || completion.stage != expected_stage
        || completion.status != "finalized"
        || completion.cluster != "owned-loopback"
        || completion.genesis_hash != expected_genesis
        || completion.sources.is_empty()
        || completion.events.is_empty()
    {
        return Err(Error::new(
            "activity stage completion is absent, partial, or another cluster",
        ));
    }
    let projection = json!({
        "accounts": completion.accounts,
        "events": completion.events,
        "finalAccounts": completion.final_accounts,
    });
    if completion.projection_sha256 != sha256(&canonical_json(&projection)?) {
        return Err(Error::new("activity stage projection digest changed"));
    }
    let mut paths = BTreeSet::new();
    let mut raw_sources = Vec::with_capacity(completion.sources.len());
    for source in &completion.sources {
        if !paths.insert(source.path.clone()) {
            return Err(Error::new("activity stage sources repeat a path"));
        }
        let path = resolve_relative(root, &source.path, "activity stage raw source")?;
        let bytes = bounded_read(&path, "activity stage raw source")?;
        let value = parse_json_without_duplicate_keys_v1(&bytes)?;
        let schema = value
            .get("schema")
            .or_else(|| value.get("format"))
            .and_then(Value::as_str);
        if sha256(&bytes) != source.sha256
            || schema != Some(source.schema.as_str())
            || normalized_scalar(json_pointer(
                &value,
                &source.completion_pointer,
                "activity stage raw source",
            )?)? != source.completion_value
        {
            return Err(Error::new(
                "activity stage raw source is missing, substituted, or incomplete",
            ));
        }
        raw_sources.push(RawSourceV1 {
            role: source.role.clone(),
            path,
            relative: source.path.clone(),
            bytes,
            value,
        });
    }
    validate_raw_source_roles(expected_stage, &raw_sources)?;
    let rederived = derive_stage_completion(expected_stage, &raw_sources, rpc, expected_genesis)?;
    exact_stage_completion_match(completion, &rederived)
}

fn exact_stage_completion_match(
    claimed: &StageCompletionV1,
    rederived: &StageCompletionV1,
) -> Result<()> {
    if claimed != rederived {
        return Err(Error::new(
            "activity stage wrapper differs from its hard-coded source adapter and finalized history",
        ));
    }
    Ok(())
}

fn merge_account_identity(
    accounts: &mut BTreeMap<String, AccountV1>,
    account: &AccountV1,
) -> Result<()> {
    match accounts.get(&account.address) {
        Some(prior) if !same_account_identity(prior, account) => Err(Error::new(
            "stage wrappers disagree on one activity account identity",
        )),
        Some(_) => Ok(()),
        None => {
            accounts.insert(account.address.clone(), account.clone());
            Ok(())
        }
    }
}

fn merge_final_account(
    accounts: &mut BTreeMap<String, FinalAccountV1>,
    account: &FinalAccountV1,
) -> Result<()> {
    match accounts.get(&account.account) {
        Some(prior) if prior != account => Err(Error::new(
            "stage wrappers disagree on one final account observation",
        )),
        Some(_) => Ok(()),
        None => {
            accounts.insert(account.account.clone(), account.clone());
            Ok(())
        }
    }
}

fn same_account_identity(left: &AccountV1, right: &AccountV1) -> bool {
    left.r#ref == right.r#ref
        && left.address == right.address
        && left.kind == right.kind
        && left.role == right.role
        && left.mint == right.mint
        && left.asset_class == right.asset_class
        && left.authority == right.authority
        && left.program_owner == right.program_owner
}

fn authenticate_plan(path: &Path) -> Result<SuccessorPlan> {
    let path = canonical_regular(path, "successor plan")?;
    let plan: SuccessorPlan =
        exact_deserialize(&bounded_read(&path, "successor plan")?, "successor plan")?;
    authenticate_checked_local_mutable_plan_v1(&plan)?;
    Ok(plan)
}

fn exact_loader_addresses(plan: &SuccessorPlan) -> Result<BTreeSet<String>> {
    let checked = plan
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("capture plan omitted checked local mutable set"))?;
    if checked.roles.len() != 7 {
        return Err(Error::new("capture plan omitted exact seven Loader pairs"));
    }
    let mut pairs = Vec::with_capacity(7);
    for role in &checked.roles {
        let program = parse_pubkey(&role.program_id, "checked program ID")?;
        let programdata = parse_pubkey(&role.programdata_id, "checked ProgramData ID")?;
        pairs.push((program, programdata));
    }
    exact_loader_pair_closure(&pairs)
}

fn exact_loader_pair_closure(pairs: &[(Pubkey, Pubkey)]) -> Result<BTreeSet<String>> {
    if pairs.len() != 7 {
        return Err(Error::new("capture plan omitted exact seven Loader pairs"));
    }
    let mut addresses = BTreeSet::new();
    for (program, programdata) in pairs {
        if get_program_data_address(program) != *programdata
            || !addresses.insert(program.to_string())
            || !addresses.insert(programdata.to_string())
        {
            return Err(Error::new("checked Loader pairs alias or have a bad link"));
        }
    }
    for program_id in [PYTH_RECEIVER_PROGRAM_ID, PYTH_ROUTER_PROGRAM_ID] {
        let program = parse_pubkey(program_id, "provider program ID")?;
        let programdata = get_program_data_address(&program);
        if !addresses.insert(program.to_string()) || !addresses.insert(programdata.to_string()) {
            return Err(Error::new("provider Loader pairs alias checked programs"));
        }
    }
    if addresses.len() != 18 {
        return Err(Error::new(
            "capture Loader closure is not exact 18 accounts",
        ));
    }
    Ok(addresses)
}

fn manifest_genesis(manifest: &ManifestV1) -> Result<String> {
    let cluster = object(Some(&manifest.cluster), "manifest cluster")?;
    if cluster.len() != 2 || cluster.get("kind").and_then(Value::as_str) != Some("owned-loopback") {
        return Err(Error::new("manifest cluster is not exact owned-loopback"));
    }
    let genesis = pubkey_field(cluster, "genesisHash", "manifest cluster")?;
    if genesis == DEVNET_GENESIS_HASH || genesis == MAINNET_BETA_GENESIS_HASH {
        return Err(Error::new("manifest cluster names a public genesis"));
    }
    Ok(genesis)
}

fn finalized_multiple_accounts(
    rpc: &mut Rpc,
    addresses: &BTreeSet<String>,
) -> Result<(u64, BTreeMap<String, Option<Value>>)> {
    if addresses.is_empty() || addresses.len() > MAX_ACCOUNTS {
        return Err(Error::new(
            "getMultipleAccounts address set is empty or exceeds 512",
        ));
    }
    let ordered = addresses.iter().cloned().collect::<Vec<_>>();
    let result = rpc.call(
        "getMultipleAccounts",
        &json!([ordered, {"commitment":"finalized", "encoding":"base64"}]),
    )?;
    parse_finalized_multiple_accounts(&result, &ordered)
}

fn parse_finalized_multiple_accounts(
    result: &Value,
    ordered: &[String],
) -> Result<(u64, BTreeMap<String, Option<Value>>)> {
    let root = object(Some(&result), "getMultipleAccounts result")?;
    if root.keys().any(|key| key != "context" && key != "value")
        || !root.contains_key("context")
        || !root.contains_key("value")
    {
        return Err(Error::new(
            "getMultipleAccounts result has unknown or missing fields",
        ));
    }
    let context = object(root.get("context"), "getMultipleAccounts context")?;
    if context
        .keys()
        .any(|key| key != "slot" && key != "apiVersion")
        || !context.contains_key("slot")
    {
        return Err(Error::new(
            "getMultipleAccounts context is partial or has unknown fields",
        ));
    }
    let slot = u64_field(context, "slot", "getMultipleAccounts context")?;
    let values = array(root.get("value"), "getMultipleAccounts values")?;
    if values.len() != ordered.len() {
        return Err(Error::new(
            "getMultipleAccounts returned a partial account vector",
        ));
    }
    let mut out = BTreeMap::new();
    for (address, value) in ordered.iter().zip(values) {
        let normalized = if value.is_null() {
            None
        } else {
            rpc_account(value, "getMultipleAccounts value")?;
            Some(value.clone())
        };
        out.insert(address.clone(), normalized);
    }
    Ok((slot, out))
}

fn rpc_account<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>> {
    let row = object(Some(value), label)?;
    let required = ["lamports", "owner", "data", "executable", "rentEpoch"];
    if required.iter().any(|field| !row.contains_key(*field))
        || row
            .keys()
            .any(|field| !required.contains(&field.as_str()) && field != "space")
    {
        return Err(Error::new(format!(
            "{label} is partial or has unknown fields"
        )));
    }
    u64_field(row, "lamports", label)?;
    pubkey_field(row, "owner", label)?;
    u64_field(row, "rentEpoch", label)?;
    if !row.get("executable").is_some_and(Value::is_boolean) {
        return Err(Error::new(format!("{label} executable is not boolean")));
    }
    let data = account_data(row, label)?;
    if let Some(space) = row.get("space") {
        let space = space
            .as_u64()
            .ok_or_else(|| Error::new(format!("{label} space is not u64")))?;
        if usize::try_from(space) != Ok(data.len()) {
            return Err(Error::new(format!(
                "{label} space differs from decoded data"
            )));
        }
    }
    Ok(row)
}

fn account_data(row: &Map<String, Value>, label: &str) -> Result<Vec<u8>> {
    let tuple = array(row.get("data"), &format!("{label} data"))?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(Error::new(format!(
            "{label} data is not exact [body, base64]"
        )));
    }
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{label} data body is not text")))?;
    let decoded = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("{label} base64: {error}")))?;
    if BASE64.encode(&decoded) != encoded {
        return Err(Error::new(format!("{label} base64 is noncanonical")));
    }
    Ok(decoded)
}

fn decode_token_account(data: &[u8]) -> Result<(String, String, u64)> {
    if data.len() < 165 || !matches!(data[108], 1 | 2) {
        return Err(Error::new(
            "token account is not a live Token-2022 base account",
        ));
    }
    let mint = Pubkey::new_from_array(data[0..32].try_into().unwrap()).to_string();
    let authority = Pubkey::new_from_array(data[32..64].try_into().unwrap()).to_string();
    let amount = u64::from_le_bytes(data[64..72].try_into().unwrap());
    Ok((mint, authority, amount))
}

fn decode_live_token_identity(value: &Value) -> Option<TokenIdentityV1> {
    let row = value.as_object()?;
    let data = account_data(row, "live token identity").ok()?;
    let (mint, authority, _) = decode_token_account(&data).ok()?;
    let program_owner = row.get("owner")?.as_str()?.to_owned();
    Pubkey::from_str(&program_owner).ok()?;
    Some(TokenIdentityV1 {
        mint,
        authority,
        program_owner,
    })
}

fn owned_loopback_origin(value: &str) -> Result<ClusterOriginV1> {
    let origin = ClusterOriginV1::parse(value, None)?;
    ExpectedClusterV1::OwnedLoopback.authenticate(&origin)?;
    Ok(origin)
}

fn owned_genesis(rpc: &mut Rpc) -> Result<String> {
    let value = rpc.call("getGenesisHash", &json!([]))?;
    let genesis = value
        .as_str()
        .ok_or_else(|| Error::new("getGenesisHash did not return text"))?;
    parse_pubkey(genesis, "owned-loopback genesis")?;
    if genesis == DEVNET_GENESIS_HASH || genesis == MAINNET_BETA_GENESIS_HASH {
        return Err(Error::new(
            "owned-loopback producer observed a public genesis",
        ));
    }
    Ok(genesis.into())
}

fn exact_deserialize<T: for<'de> Deserialize<'de>>(bytes: &[u8], label: &str) -> Result<T> {
    let value = parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    serde_json::from_value(value).map_err(|error| Error::new(format!("{label}: {error}")))
}

fn object<'a>(value: Option<&'a Value>, label: &str) -> Result<&'a Map<String, Value>> {
    value
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new(format!("{label} is not an object")))
}

fn array<'a>(value: Option<&'a Value>, label: &str) -> Result<&'a Vec<Value>> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new(format!("{label} is not an array")))
}

fn string_field<'a>(object: &'a Map<String, Value>, field: &str, label: &str) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::new(format!("{label} omitted nonempty {field}")))
}

fn pubkey_field(object: &Map<String, Value>, field: &str, label: &str) -> Result<String> {
    let value = string_field(object, field, label)?;
    parse_pubkey(value, &format!("{label} {field}"))?;
    Ok(value.into())
}

fn signature_field(object: &Map<String, Value>, field: &str, label: &str) -> Result<String> {
    let value = string_field(object, field, label)?;
    Signature::from_str(value).map_err(|error| Error::new(format!("{label} {field}: {error}")))?;
    Ok(value.into())
}

fn u64_field(object: &Map<String, Value>, field: &str, label: &str) -> Result<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("{label} {field} is not u64")))
}

fn optional_u64_field(
    object: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<Option<u64>> {
    match object.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| Error::new(format!("{label} {field} is not null or u64"))),
    }
}

fn u64_array(value: Option<&Value>, label: &str) -> Result<Vec<u64>> {
    array(value, label)?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| Error::new(format!("{label} contains a non-u64")))
        })
        .collect()
}

fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    let key = Pubkey::from_str(value).map_err(|error| Error::new(format!("{label}: {error}")))?;
    if key.to_string() != value {
        return Err(Error::new(format!("{label} is noncanonical")));
    }
    Ok(key)
}

fn canonical_decimal(value: &str, label: &str, positive: bool) -> Result<u64> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(Error::new(format!("{label} is not canonical u64 decimal")));
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    if positive && parsed == 0 {
        return Err(Error::new(format!("{label} must be positive")));
    }
    Ok(parsed)
}

fn canonical_signed_decimal(value: &str, label: &str) -> Result<i128> {
    let body = value.strip_prefix('-').unwrap_or(value);
    if body.is_empty()
        || !body.bytes().all(|byte| byte.is_ascii_digit())
        || (body.len() > 1 && body.starts_with('0'))
        || value == "-0"
    {
        return Err(Error::new(format!(
            "{label} is not canonical signed decimal"
        )));
    }
    value
        .parse::<i128>()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn normalized_scalar(value: &Value) -> Result<String> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        _ => Err(Error::new("completion value is not one canonical scalar")),
    }
}

fn json_pointer<'a>(value: &'a Value, pointer: &str, label: &str) -> Result<&'a Value> {
    if !pointer.starts_with('/') || pointer == "/" {
        return Err(Error::new(format!(
            "{label} completion pointer is not RFC6901"
        )));
    }
    let mut current = value;
    for encoded in pointer[1..].split('/') {
        let bytes = encoded.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'~'
                && (index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1'))
            {
                return Err(Error::new(format!(
                    "{label} completion pointer has a bad escape"
                )));
            }
            index += if bytes[index] == b'~' { 2 } else { 1 };
        }
        let part = encoded.replace("~1", "/").replace("~0", "~");
        current = match current {
            Value::Object(object) => object.get(&part),
            Value::Array(array) if part == "0" || !part.starts_with('0') => part
                .parse::<usize>()
                .ok()
                .and_then(|index| array.get(index)),
            _ => None,
        }
        .ok_or_else(|| Error::new(format!("{label} completion pointer is absent")))?;
    }
    Ok(current)
}

fn canonical_json(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn bounded_read(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > MAX_JSON_BYTES {
        return Err(Error::new(format!(
            "{label} must contain one through 32 MiB"
        )));
    }
    Ok(fs::read(path)?)
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} is not absolute")));
    }
    let metadata = path.symlink_metadata()?;
    let canonical = path.canonicalize()?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || canonical != path {
        return Err(Error::new(format!(
            "{label} is not one canonical ordinary directory"
        )));
    }
    Ok(canonical)
}

fn canonical_regular(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} is not absolute")));
    }
    let metadata = path.symlink_metadata()?;
    let canonical = path.canonicalize()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || canonical != path {
        return Err(Error::new(format!(
            "{label} is not one canonical ordinary file"
        )));
    }
    Ok(canonical)
}

fn canonical_relative(value: &str, label: &str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty()
        || !value.is_ascii()
        || value.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.to_str() != Some(value)
    {
        return Err(Error::new(format!(
            "{label} is not canonical relative ASCII"
        )));
    }
    Ok(())
}

fn resolve_relative(root: &Path, relative: &str, label: &str) -> Result<PathBuf> {
    canonical_relative(relative, label)?;
    let candidate = root.join(relative);
    let metadata = candidate.symlink_metadata()?;
    let canonical = candidate.canonicalize()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::new(format!("{label} is not one ordinary file")));
    }
    require_descendant(root, &canonical, label)?;
    Ok(canonical)
}

fn require_descendant(root: &Path, path: &Path, label: &str) -> Result<()> {
    path.strip_prefix(root)
        .map_err(|_| Error::new(format!("{label} escapes its evidence root")))?;
    Ok(())
}

fn write_new_json(path: &Path, value: &Value) -> Result<()> {
    let bytes = canonical_json(value)?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(Error::new("output JSON exceeds 32 MiB"));
    }
    write_new_bytes(path, &bytes)
}

fn write_new_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if !path.is_absolute() || path.exists() {
        return Err(Error::new("output must be one absent absolute path"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("output has no parent directory"))?;
    let parent = canonical_directory(parent, "output parent")?;
    if path.parent() != Some(parent.as_path()) {
        return Err(Error::new("output parent is not canonical"));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direct_trade::{
        DirectFinalizedMutationEvidenceV1, DirectPositionTransitionEvidenceV1,
        DirectTradeExpectedPoststateV1,
    };

    fn account(address: &str, role: &str) -> AccountV1 {
        AccountV1 {
            r#ref: address.into(),
            address: address.into(),
            kind: "protocol".into(),
            role: role.into(),
            mint: None,
            asset_class: None,
            authority: None,
            program_owner: None,
        }
    }

    fn final_account(address: &str, closed: bool) -> FinalAccountV1 {
        FinalAccountV1 {
            account: address.into(),
            closed,
            owner: (!closed).then(|| Pubkey::new_unique().to_string()),
            lamports: (!closed).then(|| "1".into()),
            data_sha256: (!closed).then(|| "00".repeat(32)),
            mint: None,
            authority: None,
            amount_atoms: None,
        }
    }

    fn event(retirement: Option<Value>) -> EventV1 {
        EventV1 {
            id: "retirement-000".into(),
            kind: "retirement".into(),
            operation: "aggregate-retirement".into(),
            predecessor: None,
            signature: "signature".into(),
            slot: "1".into(),
            fee_payer: Pubkey::new_unique().to_string(),
            fee_lamports: "1".into(),
            compute_units_consumed: "1".into(),
            lamport_deltas: Vec::new(),
            token_deltas: Vec::new(),
            source_path: "source.json".into(),
            source_sha256: "11".repeat(32),
            direct: None,
            positions: None,
            fee_settlement: None,
            position: None,
            certificate: None,
            payout: None,
            retirement,
        }
    }

    fn retirement_rpc_event(
        operation: AggregateRetirementOperationV1,
        closed_accounts: Vec<String>,
    ) -> RpcEventV1 {
        let mut event = event(Some(json!({
            "stage": operation.completion_name(),
            "closedAccounts": closed_accounts,
            "refundLamports": [],
        })));
        event.operation = operation.label().into();
        RpcEventV1 {
            event,
            transaction: Value::Null,
            token_identities: BTreeMap::new(),
            changed_addresses: BTreeSet::new(),
        }
    }

    fn completion() -> StageCompletionV1 {
        let mut completion = StageCompletionV1 {
            schema: STAGE_SCHEMA_V1.into(),
            stage: "founding".into(),
            status: "finalized".into(),
            cluster: "owned-loopback".into(),
            genesis_hash: Pubkey::new_unique().to_string(),
            sources: vec![SourceRefV1 {
                role: "campaign".into(),
                path: "campaign.json".into(),
                sha256: "22".repeat(32),
                schema: CAMPAIGN_SCHEMA_V1.into(),
                completion_pointer: "/execution/completed".into(),
                completion_value: "true".into(),
            }],
            accounts: vec![account("account", "protocol-account")],
            events: vec![event(None)],
            final_accounts: Vec::new(),
            projection_sha256: String::new(),
        };
        refresh_projection_digest(&mut completion);
        completion
    }

    fn refresh_projection_digest(completion: &mut StageCompletionV1) {
        completion.projection_sha256 = sha256(
            &canonical_json(&json!({
                "accounts": completion.accounts,
                "events": completion.events,
                "finalAccounts": completion.final_accounts,
            }))
            .unwrap(),
        );
    }

    fn lifecycle_descriptors(genesis: &str) -> Vec<AuthenticatedLifecycleDescriptorV1> {
        let mut descriptors = STAGES
            .into_iter()
            .enumerate()
            .map(|(index, stage)| {
                let mut completion = completion();
                completion.stage = stage.into();
                completion.genesis_hash = genesis.into();
                completion.events[0].id = format!("{stage}-000");
                completion.events[0].kind = stage.into();
                completion.events[0].operation = format!("{stage}-mutation");
                completion.events[0].predecessor = None;
                completion.events[0].signature = Signature::new_unique().to_string();
                completion.events[0].slot = (index + 1).to_string();
                if stage == "direct" {
                    completion.sources.push(SourceRefV1 {
                        role: "evidence".into(),
                        path: "direct/finalized.json".into(),
                        sha256: "55".repeat(32),
                        schema: DIRECT_EVIDENCE_SCHEMA_V1.into(),
                        completion_pointer: "/status".into(),
                        completion_value: "finalized".into(),
                    });
                } else if stage == "payout" {
                    completion.sources = vec![SourceRefV1 {
                        role: "direct-evidence".into(),
                        path: "direct/finalized.json".into(),
                        sha256: "55".repeat(32),
                        schema: DIRECT_EVIDENCE_SCHEMA_V1.into(),
                        completion_pointer: "/status".into(),
                        completion_value: "finalized".into(),
                    }];
                }
                refresh_projection_digest(&mut completion);
                AuthenticatedLifecycleDescriptorV1 {
                    descriptor: JournalDescriptorV1 {
                        semantic_role: stage.into(),
                        path: format!("stages/{index:02}-{stage}.json"),
                        schema: STAGE_SCHEMA_V1.into(),
                        completion_pointer: "/status".into(),
                    },
                    source_sha256: format!("{index:064x}"),
                    completion_value: "finalized".into(),
                    mutation_kind: None,
                    stage_completion: Some(completion),
                }
            })
            .collect::<Vec<_>>();
        for (index, (role, kind)) in [("alt", "lookup-freeze"), ("seal", "capability-seal")]
            .into_iter()
            .enumerate()
        {
            descriptors.insert(
                2 + index,
                AuthenticatedLifecycleDescriptorV1 {
                    descriptor: JournalDescriptorV1 {
                        semantic_role: role.into(),
                        path: format!("journals/{kind}.json"),
                        schema: DIRECT_OWNED_JOURNAL_SCHEMA_V1.into(),
                        completion_pointer: "/phase".into(),
                    },
                    source_sha256: format!("{:064x}", 32 + index),
                    completion_value: "finalized".into(),
                    mutation_kind: Some(kind.into()),
                    stage_completion: None,
                },
            );
        }
        descriptors.push(AuthenticatedLifecycleDescriptorV1 {
            descriptor: JournalDescriptorV1 {
                semantic_role: "direct-hot-journal".into(),
                path: "journals/direct-hot.json".into(),
                schema: "dclutch-test-direct-hot-journal-v1".into(),
                completion_pointer: "/phase".into(),
            },
            source_sha256: "aa".repeat(32),
            completion_value: "finalized".into(),
            mutation_kind: None,
            stage_completion: None,
        });
        descriptors
    }

    #[test]
    fn lifecycle_session_is_exact_eight_stage_order_with_terminal_digest() {
        let genesis = Pubkey::new_unique().to_string();
        let rows = lifecycle_descriptors(&genesis);
        let session = assemble_lifecycle_session(&genesis, &rows).expect("exact session");
        assert_eq!(session.schema, LIFECYCLE_SESSION_SCHEMA_V1);
        assert_eq!(session.status, "finalized");
        assert_eq!(session.cluster, "owned-loopback");
        assert_eq!(session.genesis_hash, genesis);
        assert_eq!(session.completed_stages, LIFECYCLE_SESSION_STAGES);
        assert_eq!(
            session.stage_set_sha256,
            sha256(
                &canonical_json(&serde_json::to_value(&session.stages).expect("stage rows"))
                    .expect("canonical stage rows")
            )
        );
    }

    #[test]
    fn lifecycle_session_refuses_missing_reordered_and_duplicate_stage_closures() {
        let genesis = Pubkey::new_unique().to_string();
        let rows = lifecycle_descriptors(&genesis);

        let mut missing = rows.clone();
        missing.remove(5);
        assert!(assemble_lifecycle_session(&genesis, &missing).is_err());

        let mut reordered = rows.clone();
        reordered.swap(1, 2);
        assert!(assemble_lifecycle_session(&genesis, &reordered).is_err());

        let mut duplicate = rows.clone();
        duplicate[1].descriptor.path = duplicate[0].descriptor.path.clone();
        assert!(assemble_lifecycle_session(&genesis, &duplicate).is_err());

        let mut repeated_mutation = rows;
        let repeated_signature = repeated_mutation[0]
            .stage_completion
            .as_ref()
            .unwrap()
            .events[0]
            .signature
            .clone();
        repeated_mutation[1]
            .stage_completion
            .as_mut()
            .unwrap()
            .events[0]
            .signature = repeated_signature;
        assert!(assemble_lifecycle_session(&genesis, &repeated_mutation).is_err());

        let mut substituted_source = lifecycle_descriptors(&genesis);
        substituted_source[6]
            .stage_completion
            .as_mut()
            .unwrap()
            .sources[0]
            .sha256 = "66".repeat(32);
        assert!(assemble_lifecycle_session(&genesis, &substituted_source).is_err());
    }

    #[test]
    fn lifecycle_session_refuses_foreign_genesis_and_false_completion() {
        let genesis = Pubkey::new_unique().to_string();
        let rows = lifecycle_descriptors(&genesis);

        let mut foreign = rows.clone();
        foreign[5].stage_completion.as_mut().unwrap().genesis_hash =
            Pubkey::new_unique().to_string();
        assert!(assemble_lifecycle_session(&genesis, &foreign).is_err());

        let mut provisional = rows;
        provisional.last_mut().unwrap().completion_value = "false".into();
        assert!(assemble_lifecycle_session(&genesis, &provisional).is_err());

        assert!(assemble_lifecycle_session(DEVNET_GENESIS_HASH, &[]).is_err());
        assert!(assemble_lifecycle_session(MAINNET_BETA_GENESIS_HASH, &[]).is_err());
    }

    fn direct_mutation(kind: &str, slot: u64) -> DirectFinalizedMutationEvidenceV1 {
        DirectFinalizedMutationEvidenceV1 {
            kind: kind.into(),
            prefix_len: None,
            path: format!("direct/{kind}.json"),
            sha256: "22".repeat(32),
            intent_sha256: "33".repeat(32),
            schema: format!("dclutch-owned-loopback-direct-{kind}-v1"),
            completion_pointer: "/phase".into(),
            completion_value: "finalized".into(),
            signature: Signature::new_unique().to_string(),
            slot,
            fee_payer: Pubkey::new_unique().to_string(),
            fee_lamports: 5_000,
            compute_units_consumed: 100_000,
        }
    }

    fn authenticated_direct() -> AuthenticatedDirectTradeEvidenceV1 {
        let seller_owner = Pubkey::new_unique();
        let buyer_owner = Pubkey::new_unique();
        let seller_position = Pubkey::new_unique();
        let buyer_position = Pubkey::new_unique();
        let buyer_maker_replay = Pubkey::new_unique();
        let poststate = |address: Pubkey| DirectTradeExpectedPoststateV1 {
            address: address.to_string(),
            owner: Pubkey::new_unique().to_string(),
            lamports: 1,
            executable: false,
            data_base64: BASE64.encode([1_u8]),
            data_sha256: "11".repeat(32),
        };
        AuthenticatedDirectTradeEvidenceV1 {
            market: Pubkey::new_unique(),
            seller_owner,
            seller_position,
            seller_collateral_destination: Pubkey::new_unique(),
            buyer_owner,
            buyer_position,
            buyer_collateral_source: Pubkey::new_unique(),
            fee_recipient: Pubkey::new_unique(),
            fee_token_account: Pubkey::new_unique(),
            mint: Pubkey::new_unique(),
            outcome_index: 1,
            outcome_count: 2,
            mutations: vec![
                direct_mutation("replay-setup", 40),
                direct_mutation("hot", 41),
            ],
            positions: [
                DirectPositionTransitionEvidenceV1 {
                    account: seller_position.to_string(),
                    owner: seller_owner.to_string(),
                    pre_data_base64: BASE64.encode([1_u8]),
                    post_data_base64: BASE64.encode([2_u8]),
                },
                DirectPositionTransitionEvidenceV1 {
                    account: buyer_position.to_string(),
                    owner: buyer_owner.to_string(),
                    pre_data_base64: BASE64.encode([3_u8]),
                    post_data_base64: BASE64.encode([4_u8]),
                },
            ],
            claim_balances: Vec::new(),
            final_accounts: vec![
                poststate(Pubkey::new_unique()),
                poststate(Pubkey::new_unique()),
                poststate(buyer_maker_replay),
            ],
            finalized_slot: 41,
            evidence_sha256: "44".repeat(32),
        }
    }

    fn direct_scalar_facts(fill: u64, scale: u64, bps: u64) -> Value {
        json!({
            "fillAtoms": fill,
            "executionPrice": 250_000,
            "priceScale": scale,
            "feeBasisPointsPerSide": bps,
        })
    }

    fn direct_fee_settlement(
        authenticated: &AuthenticatedDirectTradeEvidenceV1,
    ) -> DirectFeeSettlementEvidenceV1 {
        DirectFeeSettlementEvidenceV1 {
            schema: DIRECT_FEE_SETTLEMENT_SCHEMA_V1.into(),
            cluster: "owned-loopback".into(),
            market: authenticated.market.to_string(),
            generation: 0,
            maker: authenticated.buyer_owner.to_string(),
            maker_replay: authenticated.final_accounts[2].address.clone(),
            fee_owed: 20,
            fee_source: authenticated.buyer_collateral_source.to_string(),
            fee_destination: authenticated.fee_token_account.to_string(),
            fee_destination_owner: authenticated.fee_recipient.to_string(),
            standing_allowance: 20,
            caller_authority: Pubkey::new_unique().to_string(),
            caller_authority_bump: 1,
            custody_expected_revision: 5,
            custody_resulting_revision: 6,
            landed: Some(DirectFeeSettlementFinalizationV1 {
                signature: Signature::new_unique().to_string(),
                slot: 42,
                compute_units_consumed: Some(100_000),
                fee_lamports: Some(5_000),
            }),
        }
    }

    #[test]
    fn source_roles_refuse_duplicate_unknown_and_noncanonical_direct() {
        assert!(validate_source_roles("founding", &["campaign", "campaign"]).is_err());
        assert!(validate_source_roles("participant", &["unknown"]).is_err());
        assert!(validate_source_roles("unknown", &["campaign"]).is_err());
        assert!(validate_source_roles("direct", &[]).is_err());
        assert!(
            validate_source_roles("direct", &["evidence", "campaign", "fee-settlement"]).is_err()
        );
        assert!(validate_source_roles("direct", &["campaign", "evidence"]).is_err());
        assert!(
            validate_source_roles("direct", &["campaign", "evidence", "fee-settlement"]).is_ok()
        );
        assert!(require_frozen_stage_adapter("direct").is_ok());
    }

    #[test]
    fn direct_projection_keeps_setup_rows_observational_and_hot_semantic() {
        let authenticated = authenticated_direct();
        let facts = direct_scalar_facts(8_000, 1_000_000, 50);
        let (events, kinds) = project_authenticated_direct_activity(
            &authenticated,
            object(Some(&facts), "Direct test evidence").unwrap(),
            "direct/evidence.json",
            &"55".repeat(32),
        )
        .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].operation, "direct-replay-setup");
        assert!(events[0].direct.is_none());
        assert!(events[0].positions.is_none());
        assert_eq!(events[1].operation, "direct-hot");
        assert_eq!(events[1].expected_slot, authenticated.finalized_slot);
        assert_eq!(events[1].source_path, "direct/evidence.json");
        let direct = events[1].direct.as_ref().unwrap();
        assert_eq!(direct["fillAtoms"], "8000");
        assert_eq!(direct["executionPrice"], "250000");
        assert_eq!(direct["priceScale"], "1000000");
        assert_eq!(direct["feeBasisPointsPerSide"], "50");
        assert_eq!(
            direct["sellerToken"],
            authenticated.seller_collateral_destination.to_string()
        );
        let positions = events[1].positions.as_ref().unwrap().as_array().unwrap();
        assert_eq!(positions.len(), 2);
        assert_eq!(
            positions[0]["account"],
            authenticated.seller_position.to_string()
        );
        assert_eq!(
            positions[1]["account"],
            authenticated.buyer_position.to_string()
        );
        assert_eq!(
            kinds[&authenticated.seller_position.to_string()].0,
            "position"
        );
        assert_eq!(
            kinds[&authenticated.fee_token_account.to_string()].0,
            "token"
        );
    }

    #[test]
    fn direct_projection_refuses_noncanonical_economics_and_hot_ownership() {
        for facts in [
            direct_scalar_facts(0, 1_000_000, 50),
            direct_scalar_facts(8_000, 100, 50),
            direct_scalar_facts(8_000, 1_000_000, 49),
        ] {
            assert!(
                project_authenticated_direct_activity(
                    &authenticated_direct(),
                    object(Some(&facts), "Direct test evidence").unwrap(),
                    "direct/evidence.json",
                    &"55".repeat(32),
                )
                .is_err()
            );
        }

        let facts = direct_scalar_facts(8_000, 1_000_000, 50);
        let mut no_hot = authenticated_direct();
        no_hot.mutations[1].kind = "capability-seal".into();
        assert!(
            project_authenticated_direct_activity(
                &no_hot,
                object(Some(&facts), "Direct test evidence").unwrap(),
                "direct/evidence.json",
                &"55".repeat(32),
            )
            .is_err()
        );

        let mut two_hot = authenticated_direct();
        two_hot.mutations[0].kind = "hot".into();
        assert!(
            project_authenticated_direct_activity(
                &two_hot,
                object(Some(&facts), "Direct test evidence").unwrap(),
                "direct/evidence.json",
                &"55".repeat(32),
            )
            .is_err()
        );
    }

    #[test]
    fn direct_fee_completion_projects_exact_permissionless_obligation() {
        let authenticated = authenticated_direct();
        let facts = direct_scalar_facts(8_000, 1_000_000, 50);
        let settlement = direct_fee_settlement(&authenticated);
        let trading_program = Pubkey::new_unique().to_string();
        let pending = project_authenticated_direct_fee_settlement(
            &authenticated,
            object(Some(&facts), "Direct test evidence").unwrap(),
            &settlement,
            0,
            &trading_program,
            "direct/fee-settlement.json",
            &"66".repeat(32),
        )
        .unwrap();

        assert_eq!(pending.operation, "direct-fee-settlement");
        assert!(pending.direct.is_none());
        assert!(pending.positions.is_none());
        assert_eq!(pending.forbidden_fee_payers.len(), 3);
        let projected = pending.fee_settlement.unwrap();
        assert_eq!(projected["feeAtoms"], "20");
        assert_eq!(
            projected["sourceToken"],
            authenticated.buyer_collateral_source.to_string()
        );
        assert_eq!(
            projected["destinationToken"],
            authenticated.fee_token_account.to_string()
        );
        assert_eq!(
            projected["submissionClass"],
            "permissionless-state-derived-stranger"
        );
        assert_eq!(
            projected["capitalizationClass"],
            "debtor-collateral-obligation-not-future-revenue-or-hoard"
        );
    }

    #[test]
    fn direct_fee_completion_refuses_substitution_shortfall_and_preflight() {
        let authenticated = authenticated_direct();
        let facts = direct_scalar_facts(8_000, 1_000_000, 50);
        let trading_program = Pubkey::new_unique().to_string();
        let refuses = |settlement: &DirectFeeSettlementEvidenceV1| {
            project_authenticated_direct_fee_settlement(
                &authenticated,
                object(Some(&facts), "Direct test evidence").unwrap(),
                settlement,
                0,
                &trading_program,
                "direct/fee-settlement.json",
                &"66".repeat(32),
            )
            .is_err()
        };

        let mut wrong_fee = direct_fee_settlement(&authenticated);
        wrong_fee.fee_owed = 19;
        assert!(refuses(&wrong_fee));

        let mut wrong_source = direct_fee_settlement(&authenticated);
        wrong_source.fee_source = authenticated.seller_collateral_destination.to_string();
        assert!(refuses(&wrong_source));

        let mut short_allowance = direct_fee_settlement(&authenticated);
        short_allowance.standing_allowance = 19;
        assert!(refuses(&short_allowance));

        let mut stale_revision = direct_fee_settlement(&authenticated);
        stale_revision.custody_resulting_revision = 7;
        assert!(refuses(&stale_revision));

        let mut preflight = direct_fee_settlement(&authenticated);
        preflight.landed = None;
        assert!(refuses(&preflight));

        let mut wrong_generation = direct_fee_settlement(&authenticated);
        wrong_generation.generation = 1;
        assert!(refuses(&wrong_generation));
    }

    #[test]
    fn finalized_direct_fee_return_receipt_is_the_economic_authority() {
        let producer = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let maker_replay = Pubkey::new_unique();
        let source = Pubkey::new_unique();
        let destination = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let expected = DirectFeeReceiptExpectationV1 {
            producer: producer.to_string(),
            market: market.to_string(),
            maker: maker.to_string(),
            maker_replay: maker_replay.to_string(),
            fee_source: source.to_string(),
            fee_destination: destination.to_string(),
            fee_recipient: recipient.to_string(),
            settled_amount: 20,
            expected_revision: 5,
            resulting_revision: 6,
        };
        let receipt = DirectFeeSettlementReceiptV1 {
            request_digest: [1; 32],
            market: market.to_bytes(),
            maker: maker.to_bytes(),
            maker_root: maker_replay.to_bytes(),
            custody_replay: Pubkey::new_unique().to_bytes(),
            fee_source: source.to_bytes(),
            fee_destination: destination.to_bytes(),
            fee_recipient: recipient.to_bytes(),
            custody_request_digest: [2; 32],
            custody_poststate: [3; 32],
            settled_amount: 20,
            expected_revision: 5,
            resulting_revision: 6,
        };
        let meta = json!({
            "returnData": {
                "programId": producer.to_string(),
                "data": [BASE64.encode(receipt.to_bytes().unwrap()), "base64"],
            }
        });
        assert!(
            authenticate_direct_fee_return_data(
                object(Some(&meta), "fee receipt test").unwrap(),
                &expected,
            )
            .is_ok()
        );

        let mut substituted = expected.clone();
        substituted.settled_amount = 19;
        assert!(
            authenticate_direct_fee_return_data(
                object(Some(&meta), "fee receipt test").unwrap(),
                &substituted,
            )
            .is_err()
        );
        let malformed = json!({
            "returnData": {
                "programId": producer.to_string(),
                "data": [BASE64.encode([0_u8; 360]), "base64"],
            }
        });
        assert!(
            authenticate_direct_fee_return_data(
                object(Some(&malformed), "fee receipt test").unwrap(),
                &expected,
            )
            .is_err()
        );
    }

    #[test]
    fn payout_roles_refuse_zero_gap_and_reorder() {
        assert!(validate_source_roles("payout", &[]).is_err());
        assert!(validate_source_roles("payout", &["input-000", "evidence-001"]).is_err());
        assert!(
            validate_source_roles("payout", &["direct-evidence", "input-000", "evidence-000"])
                .is_ok()
        );
        assert!(
            validate_source_roles("payout", &["direct-evidence", "evidence-000", "input-000"])
                .is_err()
        );
        assert!(
            validate_source_roles(
                "payout",
                &["evidence-000", "input-000", "input-001", "evidence-001"],
            )
            .is_err()
        );
        assert!(
            validate_source_roles(
                "payout",
                &["input-000", "evidence-000", "input-002", "evidence-002"],
            )
            .is_err()
        );
    }

    #[test]
    fn resolution_requires_v3_submit_provider_execute_core_accept_reclaim_with_exact_compute() {
        let market = Pubkey::new_unique().to_string();
        let certificate = Pubkey::new_unique().to_string();
        let raw = |role: &str, value: Value| RawSourceV1 {
            role: role.into(),
            path: PathBuf::from(format!("/{role}.json")),
            relative: format!("{role}.json"),
            bytes: canonical_json(&value).unwrap(),
            value,
        };
        let input = raw(
            "input",
            json!({
                "format": RESOLUTION_INPUT_FORMAT_V1,
                "accounts": {"market": market, "certificate": certificate},
            }),
        );
        let receipt = |stage: &str, compute: u64| {
            json!({
                "stage": stage,
                "signature": Signature::new_unique().to_string(),
                "slot": compute,
                "feeLamports": 5_000,
                "computeUnitsConsumed": compute,
            })
        };
        let checkpoint_value = json!({
            "format": RESOLUTION_CHECKPOINT_FORMAT_V1,
            "verifiedTerminal": true,
            "receipts": [
                receipt("submit", 101),
                receipt("resolution-provider-execute-v1", 102),
                receipt("core-terminal-accept-v1", 103),
                receipt("reclaim", 104),
            ],
        });
        let checkpoint = raw("checkpoint", checkpoint_value.clone());
        let (_, pending, _) = adapt_resolution(&[input.clone(), checkpoint]).unwrap();
        assert_eq!(
            pending
                .iter()
                .map(|event| event.operation.as_str())
                .collect::<Vec<_>>(),
            [
                "resolution-submit",
                "resolution-provider-execute-v1",
                "core-terminal-accept-v1",
                "resolution-reclaim"
            ]
        );
        assert_eq!(
            pending
                .iter()
                .map(|event| event.expected_compute)
                .collect::<Vec<_>>(),
            [Some(101), Some(102), Some(103), Some(104)]
        );
        assert!(pending[0].certificate.is_none());
        assert!(pending[1].certificate.is_some());
        assert!(pending[2].certificate.is_none());
        assert!(pending[3].certificate.is_none());

        let mut old = checkpoint_value.clone();
        old["format"] =
            Value::String("dclutch-owned-loopback-flagship-resolution-checkpoint-v1".into());
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", old)]).is_err());

        let mut missing_compute = checkpoint_value.clone();
        missing_compute["receipts"][1]
            .as_object_mut()
            .unwrap()
            .remove("computeUnitsConsumed");
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", missing_compute)]).is_err());

        let mut zero_fee = checkpoint_value.clone();
        zero_fee["receipts"][0]["feeLamports"] = Value::from(0_u64);
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", zero_fee)]).is_ok());
        let mut missing_fee = checkpoint_value.clone();
        missing_fee["receipts"][0]
            .as_object_mut()
            .unwrap()
            .remove("feeLamports");
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", missing_fee)]).is_err());
        let mut boolean_fee = checkpoint_value.clone();
        boolean_fee["receipts"][0]["feeLamports"] = Value::Bool(false);
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", boolean_fee)]).is_err());

        let mut partial_execute = checkpoint_value.clone();
        partial_execute["verifiedTerminal"] = Value::Bool(false);
        partial_execute["receipts"]
            .as_array_mut()
            .unwrap()
            .truncate(2);
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", partial_execute)]).is_err());

        let mut omitted_accept = checkpoint_value.clone();
        omitted_accept["receipts"].as_array_mut().unwrap().remove(2);
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", omitted_accept)]).is_err());

        let mut replayed_accept = checkpoint_value.clone();
        let accept = replayed_accept["receipts"][2].clone();
        replayed_accept["receipts"]
            .as_array_mut()
            .unwrap()
            .insert(3, accept);
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", replayed_accept)]).is_err());

        let mut crossed_slot = checkpoint_value.clone();
        crossed_slot["receipts"][2]["slot"] = json!(102);
        assert!(adapt_resolution(&[input.clone(), raw("checkpoint", crossed_slot)]).is_err());

        let mut omitted_reclaim = checkpoint_value;
        omitted_reclaim["receipts"].as_array_mut().unwrap().pop();
        assert!(adapt_resolution(&[input, raw("checkpoint", omitted_reclaim)]).is_err());
    }

    #[test]
    fn self_consistent_projection_and_source_substitutions_are_refused() {
        let authentic = completion();
        assert!(exact_stage_completion_match(&authentic, &authentic).is_ok());

        let mut projection_substitution = authentic.clone();
        projection_substitution.accounts[0].role = "invented-role".into();
        refresh_projection_digest(&mut projection_substitution);
        assert!(exact_stage_completion_match(&projection_substitution, &authentic).is_err());

        let mut source_substitution = authentic.clone();
        source_substitution.sources[0].path = "substituted.json".into();
        source_substitution.sources[0].sha256 = "33".repeat(32);
        assert!(exact_stage_completion_match(&source_substitution, &authentic).is_err());
    }

    #[test]
    fn account_role_and_final_observation_conflicts_are_refused() {
        let mut accounts = BTreeMap::new();
        merge_account_identity(&mut accounts, &account("same", "first")).unwrap();
        assert!(merge_account_identity(&mut accounts, &account("same", "second")).is_err());

        let mut finals = BTreeMap::new();
        merge_final_account(&mut finals, &final_account("same", true)).unwrap();
        assert!(merge_final_account(&mut finals, &final_account("same", false)).is_err());
    }

    #[test]
    fn capture_transaction_refuses_null_partial_substitution_and_newer_slot() {
        let valid = json!({
            "slot": 10,
            "meta": {"err": null},
            "transaction": {"signatures": ["expected"]},
        });
        assert!(validate_captured_transaction(&valid, "expected", 10).is_ok());
        assert!(validate_captured_transaction(&Value::Null, "expected", 10).is_err());
        assert!(
            validate_captured_transaction(
                &json!({"slot": 10, "meta": {"err": null}}),
                "expected",
                10
            )
            .is_err()
        );
        assert!(validate_captured_transaction(&valid, "substituted", 10).is_err());
        assert!(validate_captured_transaction(&valid, "expected", 9).is_err());
    }

    #[test]
    fn account_capture_refuses_mixed_context_missing_rows_and_loader_null() {
        let ordered = vec!["one".to_owned()];
        let mixed = json!({
            "context": {"slot": 10, "contextSlot": 9},
            "value": [null],
        });
        assert!(parse_finalized_multiple_accounts(&mixed, &ordered).is_err());
        assert!(
            parse_finalized_multiple_accounts(
                &json!({"context": {"slot": 10}, "value": []}),
                &ordered,
            )
            .is_err()
        );

        let values = BTreeMap::from([("one".to_owned(), None)]);
        assert!(captured_account_row(&values, "missing", false, false, 10).is_err());
        assert!(captured_account_row(&values, "one", false, false, 10).is_err());
        assert!(captured_account_row(&values, "one", true, true, 10).is_err());
        assert!(captured_account_row(&values, "one", true, false, 10).is_ok());
    }

    #[test]
    fn aggregate_completion_projects_exact_four_retirement_events() {
        let keys = (0..5)
            .map(|_| Pubkey::new_unique().to_string())
            .collect::<Vec<_>>();
        let journals = AggregateRetirementOperationV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, operation)| {
                json!({
                    "operation": operation.completion_name(),
                    "journalSha256": "11".repeat(32),
                    "signature": Signature::new_unique().to_string(),
                    "finalizedSlot": 100 + index as u64,
                    "feeLamports": 5_000,
                    "computeUnitsConsumed": 200 + index as u64,
                    "packetSha256": "22".repeat(32),
                    "poststateSha256": "33".repeat(32),
                })
            })
            .collect::<Vec<_>>();
        let completion: AggregateRetirementConservationReceiptV1 = serde_json::from_value(json!({
            "schema": AGGREGATE_RETIREMENT_COMPLETION_SCHEMA_V1,
            "status": "finalized",
            "campaignSha256": "44".repeat(32),
            "market": keys[0].clone(),
            "checkpoint": keys[1].clone(),
            "rentCredit": keys[2].clone(),
            "refundWallet": keys[3].clone(),
            "payer": keys[4].clone(),
            "classifiedLamports": {
                "market": 1,
                "rentCredit": 2,
                "claimsRefund": 3,
                "custodyReplay": 4,
                "hoardVault": 5,
                "expectedRefundDelta": 15,
                "refundWalletBefore": 100,
            },
            "totalTransactionFeesLamports": 20_000,
            "terminalRefundWalletLamports": 115,
            "journals": journals,
            "receiptSha256": "55".repeat(32),
        }))
        .expect("typed completion");
        let (pending, kinds) = project_aggregate_retirement_completion(
            &completion,
            "retirement/completion.json",
            &"66".repeat(32),
        )
        .expect("four events");
        assert_eq!(
            pending
                .iter()
                .map(|event| event.operation.as_str())
                .collect::<Vec<_>>(),
            [
                "aggregate-retirement-prepare",
                "aggregate-retirement-close-vault",
                "aggregate-retirement-close-replay",
                "aggregate-retirement-finish",
            ]
        );
        assert_eq!(pending.len(), 4);
        assert_eq!(kinds.len(), 5);

        let mut old_singleton = completion.clone();
        old_singleton.journals.truncate(1);
        assert!(
            project_aggregate_retirement_completion(
                &old_singleton,
                "source.json",
                &"77".repeat(32)
            )
            .is_err()
        );
        let mut reordered = completion;
        reordered.journals.swap(1, 2);
        assert!(
            project_aggregate_retirement_completion(&reordered, "source.json", &"77".repeat(32))
                .is_err()
        );
    }

    #[test]
    fn loader_closure_refuses_count_bad_link_and_alias() {
        let mut pairs = (0..7)
            .map(|_| {
                let program = Pubkey::new_unique();
                (program, get_program_data_address(&program))
            })
            .collect::<Vec<_>>();
        assert_eq!(exact_loader_pair_closure(&pairs).unwrap().len(), 18);

        let missing = &pairs[..6];
        assert!(exact_loader_pair_closure(missing).is_err());
        let authentic = pairs[0];
        pairs[0].1 = Pubkey::new_unique();
        assert!(exact_loader_pair_closure(&pairs).is_err());
        pairs[0] = authentic;
        pairs[1] = authentic;
        assert!(exact_loader_pair_closure(&pairs).is_err());

        let expected = BTreeSet::from(["expected".to_owned()]);
        let values = BTreeMap::from([
            (
                "expected".to_owned(),
                Some(json!({"owner": bpf_loader_upgradeable::ID.to_string()})),
            ),
            (
                "unexpected-buffer".to_owned(),
                Some(json!({"owner": bpf_loader_upgradeable::ID.to_string()})),
            ),
        ]);
        assert!(refuse_unexpected_loader_accounts(&values, &expected).is_err());
        let exact = BTreeMap::from([(
            "expected".to_owned(),
            Some(json!({"owner": bpf_loader_upgradeable::ID.to_string()})),
        )]);
        refuse_unexpected_loader_accounts(&exact, &expected).expect("exact Loader closure");
    }

    #[test]
    fn retirement_closure_requires_four_rows_and_refuses_live_missing_or_duplicate_accounts() {
        let vault = Pubkey::new_unique().to_string();
        let replay = Pubkey::new_unique().to_string();
        let market = Pubkey::new_unique().to_string();
        let rows = vec![
            retirement_rpc_event(AggregateRetirementOperationV1::Prepare, Vec::new()),
            retirement_rpc_event(
                AggregateRetirementOperationV1::CloseVault,
                vec![vault.clone()],
            ),
            retirement_rpc_event(
                AggregateRetirementOperationV1::CloseReplay,
                vec![replay.clone()],
            ),
            retirement_rpc_event(AggregateRetirementOperationV1::Finish, vec![market.clone()]),
        ];
        let closed = BTreeMap::from([
            (vault.clone(), None),
            (replay.clone(), None),
            (market.clone(), None),
        ]);
        assert_eq!(
            exact_retirement_closed_accounts(&rows, &closed).unwrap(),
            BTreeSet::from([market.clone(), replay.clone(), vault.clone()]),
        );
        assert!(exact_retirement_closed_accounts(&rows[..1], &closed).is_err());
        let mut reordered = rows.clone();
        reordered.swap(1, 2);
        assert!(exact_retirement_closed_accounts(&reordered, &closed).is_err());
        let live = BTreeMap::from([
            (vault.clone(), Some(json!({}))),
            (replay.clone(), None),
            (market.clone(), None),
        ]);
        assert!(exact_retirement_closed_accounts(&rows, &live).is_err());
        assert!(exact_retirement_closed_accounts(&rows, &BTreeMap::new()).is_err());
        let mut duplicate = rows;
        duplicate[3].event.retirement = Some(json!({
            "stage": "finish",
            "closedAccounts": [vault],
            "refundLamports": [],
        }));
        assert!(exact_retirement_closed_accounts(&duplicate, &closed).is_err());
    }

    #[test]
    fn position_prestate_is_exact_and_refuses_bad_geometry() {
        let mut post = vec![0_u8; 144];
        post[..8].copy_from_slice(b"DCLLBP02");
        post[12..16].copy_from_slice(&2_u32.to_le_bytes());
        post[16..24].copy_from_slice(&5_u64.to_le_bytes());
        post[136..144].copy_from_slice(&7_u64.to_le_bytes());
        let (pre, burns) = reconstruct_position_prestate(&post, 1, 3).unwrap();
        assert_eq!(u64::from_le_bytes(pre[16..24].try_into().unwrap()), 4);
        assert_eq!(u64::from_le_bytes(pre[136..144].try_into().unwrap()), 10);
        assert_eq!(burns, ["0", "3"]);
        assert!(reconstruct_position_prestate(&post, 2, 3).is_err());
        assert!(reconstruct_position_prestate(&post[..143], 1, 3).is_err());
    }

    #[test]
    fn position_payout_replays_forward_and_refuses_substitution() {
        let mut pre = vec![0_u8; 144];
        pre[..8].copy_from_slice(b"DCLLBP02");
        pre[12..16].copy_from_slice(&2_u32.to_le_bytes());
        pre[16..24].copy_from_slice(&4_u64.to_le_bytes());
        pre[136..144].copy_from_slice(&10_u64.to_le_bytes());
        let (post, burns) = apply_position_payout(&pre, 1, 3).unwrap();
        assert_eq!(u64::from_le_bytes(post[16..24].try_into().unwrap()), 5);
        assert_eq!(u64::from_le_bytes(post[136..144].try_into().unwrap()), 7);
        assert_eq!(burns, ["0", "3"]);
        assert_eq!(reconstruct_position_prestate(&post, 1, 3).unwrap().0, pre);
        assert!(apply_position_payout(&pre, 2, 3).is_err());
        assert!(apply_position_payout(&pre, 1, 11).is_err());
        assert!(apply_position_payout(&pre, 1, 0).is_err());
    }

    #[test]
    fn payout_terminal_position_accepts_exact_live_or_closed_and_refuses_foreign_bytes() {
        let address = Pubkey::new_unique().to_string();
        let owner = Pubkey::new_unique().to_string();
        let post = vec![7_u8; 144];
        let mut event = event(None);
        event.position = Some(json!({
            "account": address,
            "owner": owner,
            "preDataBase64": BASE64.encode([6_u8; 144]),
            "postDataBase64": BASE64.encode(&post),
        }));
        let events = vec![RpcEventV1 {
            event,
            transaction: Value::Null,
            token_identities: BTreeMap::new(),
            changed_addresses: BTreeSet::new(),
        }];
        let live = json!({
            "lamports": 1,
            "owner": owner,
            "data": [BASE64.encode(&post), "base64"],
            "executable": false,
            "rentEpoch": 0,
            "space": post.len(),
        });
        assert!(
            authenticate_payout_terminal_positions(
                &events,
                &BTreeMap::from([(address.clone(), Some(live))]),
            )
            .is_ok()
        );
        assert!(
            authenticate_payout_terminal_positions(
                &events,
                &BTreeMap::from([(address.clone(), None)]),
            )
            .is_ok()
        );
        let substituted = json!({
            "lamports": 1,
            "owner": Pubkey::new_unique().to_string(),
            "data": [BASE64.encode([8_u8; 144]), "base64"],
            "executable": false,
            "rentEpoch": 0,
            "space": 144,
        });
        assert!(
            authenticate_payout_terminal_positions(
                &events,
                &BTreeMap::from([(address, Some(substituted))]),
            )
            .is_err()
        );
        assert!(authenticate_payout_terminal_positions(&events, &BTreeMap::new()).is_err());
    }

    #[test]
    fn parsers_refuse_unknown_flags_and_bad_json_pointer_escapes() {
        let arguments = [
            "--rpc-url",
            "http://127.0.0.1:8899",
            "--stage",
            "founding",
            "--evidence-root",
            "/tmp/evidence",
            "--source-descriptors",
            "/tmp/evidence/sources.json",
            "--output",
            "/tmp/output.json",
            "--invented",
            "fact",
        ]
        .into_iter()
        .map(str::to_owned);
        assert!(parse_stage_args(arguments).is_err());
        assert!(json_pointer(&json!({"bad~escape": true}), "/bad~2escape", "test").is_err());
        assert!(canonical_relative(r"journals\substituted.json", "test").is_err());

        let mut descriptor = JournalDescriptorV1 {
            semantic_role: "founding".into(),
            path: "founding.json".into(),
            schema: STAGE_SCHEMA_V1.into(),
            completion_pointer: "/status".into(),
        };
        authenticate_normalized_stage_descriptor(&descriptor).expect("top-level stage status");
        descriptor.completion_pointer = "/sources/0/completionValue".into();
        assert!(authenticate_normalized_stage_descriptor(&descriptor).is_err());
    }

    #[test]
    fn founding_activity_requires_split_success_order_and_refuses_legacy_atomic_label() {
        authenticate_founding_success_mutations(&FOUNDING_SUCCESS_MUTATIONS)
            .expect("exact prepare, stage, Open order");
        let reordered = [
            FOUNDING_SUCCESS_MUTATIONS[1],
            FOUNDING_SUCCESS_MUTATIONS[0],
            FOUNDING_SUCCESS_MUTATIONS[2],
            FOUNDING_SUCCESS_MUTATIONS[3],
            FOUNDING_SUCCESS_MUTATIONS[4],
            FOUNDING_SUCCESS_MUTATIONS[5],
        ];
        assert!(authenticate_founding_success_mutations(&reordered).is_err());
        assert!(authenticate_founding_success_mutations(&FOUNDING_SUCCESS_MUTATIONS[..5]).is_err());
        assert!(!is_authorized_founding_label(
            "create projected custody and controller funding (DCLTPCB2)"
        ));
        assert!(
            FOUNDING_SUCCESS_MUTATIONS
                .iter()
                .all(|label| is_authorized_founding_label(label))
        );
    }
}
