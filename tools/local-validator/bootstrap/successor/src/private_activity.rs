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
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};
use solana_loader_v3_interface::get_program_data_address;
use solana_sdk::{pubkey::Pubkey, signature::Signature};

use crate::{
    Error, Result,
    campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1,
    cluster::{ClusterOriginV1, DEVNET_GENESIS_HASH, ExpectedClusterV1, MAINNET_BETA_GENESIS_HASH},
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
const CAMPAIGN_SCHEMA_V1: &str = "dclutch-successor-campaign-report-v1";
const PARTICIPANT_SCHEMA_V1: &str = "dclutch-owned-loopback-user-position-admission-execution-v1";
const RESOLUTION_INPUT_FORMAT_V1: &str = "dclutch-owned-loopback-flagship-resolution-input-v1";
const RESOLUTION_CHECKPOINT_FORMAT_V1: &str =
    "dclutch-owned-loopback-flagship-resolution-checkpoint-v1";
const PAYOUT_INPUT_FORMAT_V1: &str = "dclutch-wallet-terminal-payout-plan-input-v1";
const PAYOUT_EVIDENCE_SCHEMA_V1: &str =
    "dclutch-local-private-validator-wallet-terminal-payout-evidence-v1";
const RETIREMENT_COMPLETION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-terminal-sequence-completion-v1";
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
    lamport_deltas: Vec<DeltaV1>,
    token_deltas: Vec<DeltaV1>,
    source_path: String,
    source_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    direct: Option<Value>,
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
    position: Option<Value>,
    certificate: Option<Value>,
    payout: Option<Value>,
    retirement: Option<Value>,
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
    owned_loopback_origin(&arguments.rpc_url)?;
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
    for descriptor in &descriptors.journals {
        if descriptor.semantic_role.is_empty()
            || descriptor.schema.is_empty()
            || !paths.insert(descriptor.path.clone())
        {
            return Err(Error::new(
                "lifecycle stage journal descriptors repeat a path or contain an empty identity",
            ));
        }
        let path = resolve_relative(&root, &descriptor.path, "lifecycle journal source")?;
        let value = parse_json_without_duplicate_keys_v1(&bounded_read(
            &path,
            "lifecycle journal source",
        )?)?;
        json_pointer(
            &value,
            &descriptor.completion_pointer,
            "lifecycle completion pointer",
        )?;
    }
    let _ = (&arguments.output, LIFECYCLE_SESSION_SCHEMA_V1);
    Err(Error::new(
        "owned-loopback lifecycle session is refused until the alt, seal, and Direct semantic-owner ABIs are frozen",
    ))
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
        "founding" | "participant" | "resolution" | "payout" | "retirement" => Ok(()),
        "direct" => Err(Error::new(
            "owned-loopback Direct activity projection is refused until the Direct finalized ABI is frozen",
        )),
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
        "resolution" => adapt_resolution(sources)?,
        "payout" => adapt_payout(sources)?,
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
            if roles.is_empty() || roles.len() % 2 != 0 || roles.len() > 64 {
                return Err(Error::new(
                    "payout sources must be one through 32 canonical input/evidence pairs",
                ));
            }
            for (index, pair) in roles.chunks_exact(2).enumerate() {
                if pair != [format!("input-{index:03}"), format!("evidence-{index:03}")] {
                    return Err(Error::new(
                        "payout source roles are not canonical input-NNN/evidence-NNN pairs",
                    ));
                }
            }
        }
        "direct" => {
            return Err(Error::new(
                "owned-loopback Direct activity projection is refused until the Direct finalized ABI is frozen",
            ));
        }
        "founding" | "participant" | "resolution" | "retirement" => {}
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
        let fee = u64_field(row, "fee_lamports", "campaign transaction")?;
        let compute = optional_u64_field(row, "compute_units_consumed", "campaign transaction")?;
        if row.get("transaction_metadata_available") != Some(&Value::Bool(true))
            || fee == 0
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
            position: None,
            certificate: None,
            payout: None,
            retirement: None,
        });
    }
    if !labels.contains("create projected custody and controller funding (DCLTPCB2)")
        || !labels
            .contains("found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF2)")
    {
        return Err(Error::new(
            "founding history omitted its exact DCLTPCB2 or DCLTGMF2 success",
        ));
    }
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
            | "create projected custody and controller funding (DCLTPCB2)"
            | "pre-fund the founding's five program-allocated accounts"
            | "found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF2)"
    ) || label.starts_with("publish record: ")
        || label.starts_with("publish Product graph: ")
        || label.starts_with("create DCLTPCB2 routing address lookup table")
        || label.starts_with("extend DCLTPCB2 routing table page ")
        || label.starts_with("create DCLTGMF2 routing address lookup table")
        || label.starts_with("extend DCLTGMF2 routing table page ")
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
    if receipts.len() != 2 {
        return Err(Error::new(
            "terminal resolution requires exact submit and execute receipts",
        ));
    }
    let mut pending = Vec::new();
    for (index, (raw, expected_stage)) in receipts.iter().zip(["submit", "execute"]).enumerate() {
        let row = object(Some(raw), "resolution receipt")?;
        if row.get("stage").and_then(Value::as_str) != Some(expected_stage) {
            return Err(Error::new(
                "resolution receipts are not exact submit then execute",
            ));
        }
        pending.push(PendingEventV1 {
            operation: format!("resolution-{expected_stage}"),
            signature: signature_field(row, "signature", "resolution receipt")?,
            expected_slot: u64_field(row, "slot", "resolution receipt")?,
            expected_fee: u64_field(row, "feeLamports", "resolution receipt")?,
            expected_compute: None,
            source_path: checkpoint.relative.clone(),
            source_sha256: sha256(&checkpoint.bytes),
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
) -> Result<(
    Vec<SourceRefV1>,
    Vec<PendingEventV1>,
    BTreeMap<String, (String, String)>,
)> {
    let mut refs = Vec::with_capacity(sources.len());
    let mut pending = Vec::with_capacity(sources.len() / 2);
    let mut kinds = BTreeMap::new();
    for (index, pair) in sources.chunks_exact(2).enumerate() {
        let input = &pair[0];
        let evidence = &pair[1];
        let input_ref = source_ref(
            input,
            PAYOUT_INPUT_FORMAT_V1,
            "format",
            "/format",
            &Value::String(PAYOUT_INPUT_FORMAT_V1.into()),
        )?;
        let evidence_object = object(Some(&evidence.value), "payout evidence")?;
        let signature = signature_field(evidence_object, "signature", "payout evidence")?;
        let evidence_ref = source_ref(
            evidence,
            PAYOUT_EVIDENCE_SCHEMA_V1,
            "schema",
            "/signature",
            &Value::String(signature.clone()),
        )?;
        let input_value: PlanInputV1 = exact_deserialize(&input.bytes, "payout input")?;
        let selected = SelectedInputV1::parse(&input_value, LookupTableRequirementV1::Present)?;
        if sha256(&input.bytes) != string_field(evidence_object, "inputSha256", "payout evidence")?
            || selected.market.to_string()
                != pubkey_field(evidence_object, "market", "payout evidence")?
            || selected.owner.to_string()
                != pubkey_field(evidence_object, "owner", "payout evidence")?
            || selected.recipient.to_string()
                != pubkey_field(evidence_object, "recipient", "payout evidence")?
        {
            return Err(Error::new(
                "payout input/evidence pair differs in bytes, Market, owner, or recipient",
            ));
        }
        let quantity = canonical_decimal(&input_value.quantity, "payout quantity", true)?;
        let principal = canonical_decimal(
            string_field(evidence_object, "payout", "payout evidence")?,
            "payout principal",
            false,
        )?;
        let position = selected.position.to_string();
        let hoard = selected.hoard.to_string();
        let recipient = selected.recipient.to_string();
        let mint = input_value.collateral_mint.clone();
        pending.push(PendingEventV1 {
            operation: format!("wallet-terminal-payout-{index:03}"),
            signature,
            expected_slot: u64_field(evidence_object, "finalizedSlot", "payout evidence")?,
            expected_fee: u64_field(evidence_object, "feeLamports", "payout evidence")?,
            expected_compute: Some(u64_field(
                evidence_object,
                "computeUnitsConsumed",
                "payout evidence",
            )?),
            source_path: evidence.relative.clone(),
            source_sha256: sha256(&evidence.bytes),
            position: None,
            certificate: None,
            payout: Some(json!({
                "_positionAddress": position,
                "_claimIndex": input_value.claim_index,
                "_quantity": quantity.to_string(),
                "hoardToken": hoard,
                "recipientToken": recipient,
                "principalAtoms": principal.to_string(),
                "mint": mint,
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
        RETIREMENT_COMPLETION_SCHEMA_V1,
        "schema",
        "/status",
        &Value::String("finalized".into()),
    )?;
    let root = object(Some(&source.value), "terminal completion")?;
    if root.get("cluster").and_then(Value::as_str) != Some("owned-loopback") {
        return Err(Error::new("terminal completion is not owned-loopback"));
    }
    let rows = array(root.get("journals"), "terminal completion journals")?;
    let admitted = [
        "resolution-receipt-prepay",
        "core-begin-retiring",
        "direct-begin-retiring",
        "resolution-close-fund",
        "direct-close-capability",
        "retirement-replay-handoff",
        "aggregate-retirement",
    ];
    let mut last = None;
    let mut pending = Vec::new();
    let mut kinds = BTreeMap::new();
    for raw in rows {
        let row = object(Some(raw), "terminal completion journal")?;
        let mutation = object(row.get("mutation"), "terminal mutation")?;
        let kind = string_field(mutation, "kind", "terminal mutation")?;
        let Some(index) = admitted.iter().position(|candidate| *candidate == kind) else {
            if kind.starts_with("lookup-") {
                continue;
            }
            return Err(Error::new(
                "terminal completion invented a retirement mutation",
            ));
        };
        if last.is_some_and(|prior| index <= prior) {
            return Err(Error::new(
                "terminal protocol mutations are duplicated or reordered",
            ));
        }
        last = Some(index);
        if row.get("phase").and_then(Value::as_str) != Some("finalized") {
            return Err(Error::new(
                "terminal completion includes a provisional protocol journal",
            ));
        }
        let mut negative = Vec::new();
        let mut refunds = Vec::new();
        for raw_delta in array(row.get("protocolLamportDeltas"), "protocol lamport deltas")? {
            let delta = object(Some(raw_delta), "protocol lamport delta")?;
            let address = pubkey_field(delta, "accountAddress", "protocol lamport delta")?;
            let amount = canonical_signed_decimal(
                string_field(delta, "deltaLamports", "protocol lamport delta")?,
                "protocol lamport delta",
            )?;
            kinds.insert(
                address.clone(),
                ("protocol".into(), format!("retirement-{kind}")),
            );
            if amount < 0 {
                negative.push(address);
            } else if amount > 0 {
                refunds.push(json!({"account": address, "lamports": amount.to_string()}));
            }
        }
        let payer = pubkey_field(row, "feePayer", "terminal completion journal")?;
        kinds.insert(payer, ("wallet".into(), "terminal-fee-payer".into()));
        pending.push(PendingEventV1 {
            operation: kind.into(),
            signature: signature_field(row, "signature", "terminal completion journal")?,
            expected_slot: canonical_decimal(
                string_field(row, "finalizedSlot", "terminal completion journal")?,
                "terminal finalized slot",
                false,
            )?,
            expected_fee: canonical_decimal(
                string_field(row, "transactionFeeLamports", "terminal completion journal")?,
                "terminal fee",
                false,
            )?,
            expected_compute: Some(canonical_decimal(
                string_field(row, "computeUnitsConsumed", "terminal completion journal")?,
                "terminal compute",
                true,
            )?),
            source_path: source.relative.clone(),
            source_sha256: sha256(&source.bytes),
            position: None,
            certificate: None,
            payout: None,
            retirement: Some(json!({
                "stage": kind,
                "_negativeCandidates": negative,
                "refundLamports": refunds,
            })),
        });
    }
    if last != Some(admitted.len() - 1) {
        return Err(Error::new(
            "terminal completion omitted aggregate retirement",
        ));
    }
    Ok((vec![source_ref], pending, kinds))
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
            lamport_deltas: lamports,
            token_deltas,
            source_path: pending.source_path,
            source_sha256: pending.source_sha256,
            direct: None,
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
        "retirement" => retirement_closed,
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

fn exact_retirement_closed_accounts(
    events: &[RpcEventV1],
    current: &BTreeMap<String, Option<Value>>,
) -> Result<BTreeSet<String>> {
    let mut closed = BTreeSet::new();
    for observed in events {
        let retirement = object(
            observed.event.retirement.as_ref(),
            "finalized retirement projection",
        )?;
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
        let account = current
            .get(&position)
            .and_then(|value| value.as_ref())
            .ok_or_else(|| Error::new("payout Position is absent after finalized mutation"))?;
        let row = rpc_account(account, "payout Position")?;
        let post = account_data(row, "payout Position")?;
        let (pre, burns) = reconstruct_position_prestate(&post, claim_index, quantity)?;
        event.position = Some(json!({
            "account": position,
            "preDataBase64": BASE64.encode(pre),
            "postDataBase64": BASE64.encode(&post),
        }));
        event.payout = Some(json!({
            "hoardToken": pubkey_field(value, "hoardToken", "pending payout")?,
            "recipientToken": pubkey_field(value, "recipientToken", "pending payout")?,
            "position": position,
            "principalAtoms": principal.to_string(),
            "claimsBurnedAtoms": burns,
            "mint": pubkey_field(value, "mint", "pending payout")?,
        }));
    }
    if let Some(raw) = event.retirement.take() {
        let value = object(Some(&raw), "pending retirement")?;
        let negative = array(value.get("_negativeCandidates"), "retirement candidates")?;
        let mut closed = Vec::new();
        for raw in negative {
            let address = raw
                .as_str()
                .ok_or_else(|| Error::new("retirement closure candidate is not text"))?;
            if current.get(address).is_some_and(Option::is_none) {
                closed.push(address.to_owned());
            }
        }
        event.retirement = Some(json!({
            "stage": string_field(value, "stage", "pending retirement")?,
            "closedAccounts": closed,
            "refundLamports": value.get("refundLamports").cloned().ok_or_else(|| Error::new("pending retirement omitted refunds"))?,
        }));
    }
    Ok(())
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
            if !STAGES.contains(&descriptor.semantic_role.as_str()) {
                return Err(Error::new(
                    "activity stage wrapper is owned by an unknown semantic role",
                ));
            }
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
            lamport_deltas: Vec::new(),
            token_deltas: Vec::new(),
            source_path: "source.json".into(),
            source_sha256: "11".repeat(32),
            direct: None,
            position: None,
            certificate: None,
            payout: None,
            retirement,
        }
    }

    fn rpc_event(retirement: Value) -> RpcEventV1 {
        RpcEventV1 {
            event: event(Some(retirement)),
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

    #[test]
    fn source_roles_refuse_duplicate_unknown_and_unfrozen_direct() {
        assert!(validate_source_roles("founding", &["campaign", "campaign"]).is_err());
        assert!(validate_source_roles("participant", &["unknown"]).is_err());
        assert!(validate_source_roles("unknown", &["campaign"]).is_err());
        assert!(validate_source_roles("direct", &[]).is_err());
        assert!(require_frozen_stage_adapter("direct").is_err());
    }

    #[test]
    fn payout_roles_refuse_zero_gap_and_reorder() {
        assert!(validate_source_roles("payout", &[]).is_err());
        assert!(validate_source_roles("payout", &["input-000", "evidence-001"]).is_err());
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
    }

    #[test]
    fn retirement_closure_refuses_live_missing_and_duplicate_accounts() {
        let address = Pubkey::new_unique().to_string();
        let row = rpc_event(json!({
            "stage": "aggregate-retirement",
            "closedAccounts": [address],
            "refundLamports": [],
        }));
        let closed = BTreeMap::from([(address.clone(), None)]);
        assert_eq!(
            exact_retirement_closed_accounts(&[row.clone()], &closed)
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            [address.clone()]
        );
        let live = BTreeMap::from([(address.clone(), Some(json!({})))]);
        assert!(exact_retirement_closed_accounts(&[row.clone()], &live).is_err());
        assert!(exact_retirement_closed_accounts(&[row.clone()], &BTreeMap::new()).is_err());
        assert!(exact_retirement_closed_accounts(&[row.clone(), row], &closed).is_err());
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
    }
}
