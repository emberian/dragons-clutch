//! Aggregate, offline authenticator for one complete owned-loopback lifecycle.
//!
//! This producer does not treat an activity descriptor as authority for deployed
//! programs.  The seven dClutch Loader pairs come from the already-authenticated
//! checked-local plan; the two provider pairs come from their fixed program IDs
//! and finalized captured account bytes.  A receipt is not written until a Pyth
//! semantic owner also binds those provider bytes and ELF tails.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Component, Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;

use crate::{
    Error, Result,
    model::SuccessorPlan,
    terminal_exterior_pyth::{
        authenticate_provider_closure_receipt_v1,
        owned_loopback_capture::{self, CaptureV1, DeploymentSlotPolicyV1},
    },
};

pub(crate) const COMMAND_V1: &str = "local-private-validator-lifecycle-receipt-v1";
const RECEIPT_SCHEMA_V1: &str = "dclutch-owned-loopback-reconcile-session-receipt-v1";
const MANIFEST_SCHEMA_V1: &str = "dclutch-owned-loopback-activity-reconcile-manifest-v1";
const DESCRIPTORS_SCHEMA_V1: &str = "dclutch-owned-loopback-stage-journal-descriptors-v1";
const ACTIVITY_SESSION_SCHEMA_V1: &str = "dclutch-owned-loopback-private-lifecycle-session-v1";
const CHAOS_SESSION_SCHEMA_V1: &str = "dclutch-owned-loopback-private-lifecycle-chaos-session-v1";
const PYTH_FACTS_SCHEMA_V1: &str = "dclutch-flagship-pyth-update-facts-v1";
const PYTH_RECEIVER_PROGRAM_ID: &str = "rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp";
const PYTH_ROUTER_PROGRAM_ID: &str = "HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL";
const MAX_INPUT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_JOURNALS: usize = 160;

const PROGRAM_ROLES: [&str; 9] = [
    "registry",
    "rent",
    "custody",
    "resolution",
    "claims",
    "trading",
    "core",
    "pyth-receiver",
    "pyth-router",
];
const ACTIVITY_STAGES: [&str; 8] = [
    "founding",
    "participant",
    "alt",
    "seal",
    "direct",
    "resolution",
    "payout",
    "retirement",
];
const MANIFEST_EVENT_KINDS: [&str; 6] = [
    "founding",
    "participant",
    "direct",
    "resolution",
    "payout",
    "retirement",
];
const CHAOS_STAGES: [&str; 8] = [
    "founding",
    "participant",
    "alt",
    "seal",
    "hot",
    "resolution",
    "payout",
    "retire",
];
const DESCRIPTOR_ROLES: [&str; 11] = [
    "founding",
    "participant",
    "alt",
    "seal",
    "direct",
    "pyth",
    "resolution",
    "payout",
    "retirement",
    "activity-session",
    "chaos-session",
];

#[derive(Clone, Debug)]
pub(crate) struct PrivateLifecycleArgs {
    evidence_root: PathBuf,
    source_commit: String,
    checked_release_gate_sha256: String,
    plan: PathBuf,
    checked_release_gate: PathBuf,
    pyth_facts: PathBuf,
    pyth_provider_closure: PathBuf,
    finalized_capture: PathBuf,
    activity_manifest: PathBuf,
    stage_journal_descriptors: PathBuf,
    chaos_session: PathBuf,
    output: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorDocument {
    schema: String,
    journals: Vec<DescriptorRow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DescriptorRow {
    semantic_role: String,
    path: String,
    schema: String,
    completion_pointer: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramReceipt {
    role: String,
    program_id: String,
    program_data_address: String,
    deployment_slot: String,
    elf_sha256: String,
    genesis_program_data_sha256: String,
    upgrade_authority: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JournalReceipt {
    path: String,
    sha256: String,
    schema: String,
    completion_pointer: String,
    completion_value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivitySessionStage {
    stage: String,
    path: String,
    sha256: String,
    schema: String,
    completion_pointer: String,
    completion_value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivitySession {
    schema: String,
    status: String,
    cluster: String,
    genesis_hash: String,
    stages: Vec<ActivitySessionStage>,
    completed_stages: Vec<String>,
    stage_set_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChaosSession {
    schema: String,
    status: String,
    stages: Vec<ChaosStage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ChaosStage {
    stage: String,
    status: String,
    intent_sha256: String,
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap local-private-validator-lifecycle-receipt-v1 \\\n     --evidence-root ABSOLUTE_DIR --source-commit FULL_LOWERCASE_COMMIT \\\n     --checked-release-gate-sha256 SHA256 --plan ABSOLUTE_JSON \\\n     --checked-release-gate ABSOLUTE_JSON --pyth-facts ABSOLUTE_JSON \\\n     --pyth-provider-closure ABSOLUTE_JSON --finalized-capture ABSOLUTE_JSON \\\n     --activity-manifest ABSOLUTE_JSON --stage-journal-descriptors ABSOLUTE_JSON \\\n     --chaos-session ABSOLUTE_JSON --output ABSOLUTE_NEW_JSON\n"
}

pub(crate) fn parse_args<I>(arguments: I) -> Result<PrivateLifecycleArgs>
where
    I: IntoIterator<Item = String>,
{
    let mut evidence_root = None;
    let mut source_commit = None;
    let mut checked_release_gate_sha256 = None;
    let mut plan = None;
    let mut checked_release_gate = None;
    let mut pyth_facts = None;
    let mut pyth_provider_closure = None;
    let mut finalized_capture = None;
    let mut activity_manifest = None;
    let mut stage_journal_descriptors = None;
    let mut chaos_session = None;
    let mut output = None;
    let mut values = arguments.into_iter();
    while let Some(flag) = values.next() {
        let value = values
            .next()
            .ok_or_else(|| Error::new(format!("{flag} omitted its value")))?;
        match flag.as_str() {
            "--evidence-root" => set_once(&mut evidence_root, PathBuf::from(value), &flag)?,
            "--source-commit" => set_once(&mut source_commit, value, &flag)?,
            "--checked-release-gate-sha256" => {
                set_once(&mut checked_release_gate_sha256, value, &flag)?
            }
            "--plan" => set_once(&mut plan, PathBuf::from(value), &flag)?,
            "--checked-release-gate" => {
                set_once(&mut checked_release_gate, PathBuf::from(value), &flag)?
            }
            "--pyth-facts" => set_once(&mut pyth_facts, PathBuf::from(value), &flag)?,
            "--pyth-provider-closure" => {
                set_once(&mut pyth_provider_closure, PathBuf::from(value), &flag)?
            }
            "--finalized-capture" => set_once(&mut finalized_capture, PathBuf::from(value), &flag)?,
            "--activity-manifest" => set_once(&mut activity_manifest, PathBuf::from(value), &flag)?,
            "--stage-journal-descriptors" => {
                set_once(&mut stage_journal_descriptors, PathBuf::from(value), &flag)?
            }
            "--chaos-session" => set_once(&mut chaos_session, PathBuf::from(value), &flag)?,
            "--output" => set_once(&mut output, PathBuf::from(value), &flag)?,
            _ => return Err(Error::new(format!("unknown private lifecycle flag {flag}"))),
        }
    }
    Ok(PrivateLifecycleArgs {
        evidence_root: required(evidence_root, "--evidence-root")?,
        source_commit: required(source_commit, "--source-commit")?,
        checked_release_gate_sha256: required(
            checked_release_gate_sha256,
            "--checked-release-gate-sha256",
        )?,
        plan: required(plan, "--plan")?,
        checked_release_gate: required(checked_release_gate, "--checked-release-gate")?,
        pyth_facts: required(pyth_facts, "--pyth-facts")?,
        pyth_provider_closure: required(pyth_provider_closure, "--pyth-provider-closure")?,
        finalized_capture: required(finalized_capture, "--finalized-capture")?,
        activity_manifest: required(activity_manifest, "--activity-manifest")?,
        stage_journal_descriptors: required(
            stage_journal_descriptors,
            "--stage-journal-descriptors",
        )?,
        chaos_session: required(chaos_session, "--chaos-session")?,
        output: required(output, "--output")?,
    })
}

pub(crate) fn run(arguments: PrivateLifecycleArgs) -> Result<Value> {
    exact_lower_hex(&arguments.source_commit, 40, "source commit")?;
    exact_lower_hex(
        &arguments.checked_release_gate_sha256,
        64,
        "checked release gate SHA-256",
    )?;
    let evidence_root = canonical_directory(&arguments.evidence_root, "evidence root")?;
    let plan_path = canonical_regular(&arguments.plan, "successor plan")?;
    let gate_path = canonical_regular(&arguments.checked_release_gate, "checked release gate")?;
    let pyth_facts_path = canonical_regular(&arguments.pyth_facts, "Pyth facts")?;
    let pyth_provider_closure_path =
        canonical_regular(&arguments.pyth_provider_closure, "Pyth provider closure")?;
    let capture_path = canonical_regular(&arguments.finalized_capture, "finalized capture")?;
    let manifest_path = canonical_regular(&arguments.activity_manifest, "activity manifest")?;
    let descriptors_path = canonical_regular(
        &arguments.stage_journal_descriptors,
        "stage journal descriptors",
    )?;
    let chaos_path = canonical_regular(&arguments.chaos_session, "chaos session")?;

    let gate_bytes = bounded_read(&gate_path, "checked release gate")?;
    let gate_sha256 = sha256(&gate_bytes);
    if gate_sha256 != arguments.checked_release_gate_sha256 {
        return Err(Error::new(
            "checked release gate bytes differ from --checked-release-gate-sha256",
        ));
    }

    let plan_bytes = bounded_read(&plan_path, "successor plan")?;
    let plan: SuccessorPlan = serde_json::from_value(exact_json(&plan_bytes, "successor plan")?)?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let checked = plan.checked_local_mutable_set.as_ref().ok_or_else(|| {
        Error::new("successor plan omitted checked local mutable release evidence")
    })?;
    if checked.source_revision != arguments.source_commit
        || checked.checked_release_gate_sha256 != gate_sha256
        || Path::new(&checked.checked_release_gate_path) != gate_path
    {
        return Err(Error::new(
            "successor plan source commit or checked release gate differs from the selected inputs",
        ));
    }
    let retained_authority = parse_pubkey(
        &checked.retained_upgrade_authority,
        "retained disposable upgrade authority",
    )?;

    // Parse the gate independently for duplicate/trailing-byte refusal.  Role
    // digest ownership remains in authenticate_checked_local_mutable_plan_v1.
    let _gate = exact_json(&gate_bytes, "checked release gate")?;

    let capture = owned_loopback_capture::authenticate_v1(&capture_path)?;
    let finalized_slot = capture.finalized_slot;

    let manifest_bytes = bounded_read(&manifest_path, "activity manifest")?;
    let manifest = exact_json(&manifest_bytes, "activity manifest")?;
    let (cluster, event_sources) = authenticate_manifest(&manifest, &capture.genesis_hash)?;

    let descriptors_bytes = bounded_read(&descriptors_path, "stage journal descriptors")?;
    let descriptors: DescriptorDocument =
        serde_json::from_value(exact_json(&descriptors_bytes, "stage journal descriptors")?)?;
    let (journals, semantic_paths) = authenticate_journals(&evidence_root, descriptors)?;
    let journal_sources = journals
        .iter()
        .map(|row| (row.path.clone(), row.sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    for (path, sha256) in &event_sources {
        if journal_sources.get(path) != Some(sha256) {
            return Err(Error::new(
                "stage journal descriptors omit or substitute an activity-manifest source journal",
            ));
        }
    }

    let chaos_relative = relative_evidence_path(&evidence_root, &chaos_path, "chaos session")?;
    if semantic_paths.get("chaos-session") != Some(&chaos_relative) {
        return Err(Error::new(
            "the chaos-session descriptor does not name --chaos-session",
        ));
    }
    let chaos = authenticate_chaos(&chaos_path)?;
    let chaos_journal = journals
        .iter()
        .find(|row| row.path == chaos_relative)
        .ok_or_else(|| Error::new("chaos-session journal was not projected"))?;
    authenticate_session_journal_identity(chaos_journal, CHAOS_SESSION_SCHEMA_V1, "chaos session")?;

    let programs = authenticate_programs(&plan, &capture, retained_authority)?;
    let capture_relative = relative_evidence_path(&evidence_root, &capture_path, "capture")?;

    authenticate_pyth_update_facts(&pyth_facts_path)?;
    let provider_closure = authenticate_provider_closure_receipt_v1(&pyth_provider_closure_path)?;
    authenticate_provider_closure_binding(&provider_closure, &capture, &capture_path, &programs)?;
    let provider_closure_bytes =
        bounded_read(&pyth_provider_closure_path, "Pyth provider closure")?;
    let provider_closure_relative = relative_evidence_path(
        &evidence_root,
        &pyth_provider_closure_path,
        "Pyth provider closure",
    )?;

    let journal_set_sha256 = sha256(&canonical_json_bytes(&serde_json::to_value(&journals)?)?);
    let session_relative = semantic_paths
        .get("activity-session")
        .ok_or_else(|| Error::new("activity-session journal descriptor is absent"))?;
    let session = journals
        .iter()
        .find(|row| &row.path == session_relative)
        .ok_or_else(|| Error::new("activity-session journal was not projected"))?;
    authenticate_session_journal_identity(session, ACTIVITY_SESSION_SCHEMA_V1, "activity session")?;
    let completed_stages = authenticate_activity_session(
        &evidence_root,
        session_relative,
        &journals,
        &capture.genesis_hash,
    )?;
    let receipt = json!({
        "schema": RECEIPT_SCHEMA_V1,
        "status": "finalized",
        "cluster": cluster,
        "sourceCommit": arguments.source_commit,
        "checkedReleaseGateSha256": gate_sha256,
        "programs": programs,
        "manifestSha256": sha256(&manifest_bytes),
        "capture": {
            "path": capture_relative,
            "sha256": capture.sha256,
            "schema": owned_loopback_capture::SCHEMA_V1,
            "commitment": "finalized",
            "finalizedSlot": finalized_slot.to_string(),
        },
        "providerClosure": {
            "path": provider_closure_relative,
            "sha256": sha256(&provider_closure_bytes),
            "schema": crate::terminal_exterior_pyth::PROVIDER_CLOSURE_SCHEMA_V1,
        },
        "journals": journals,
        "journalSetSha256": journal_set_sha256,
        "privateSession": {
            "path": session.path,
            "sha256": session.sha256,
            "schema": session.schema,
            "status": "finalized",
            "completedStages": completed_stages,
        },
        "chaosSession": {
            "path": chaos_journal.path,
            "sha256": chaos_journal.sha256,
            "schema": chaos.schema,
            "status": chaos.status,
        },
    });
    write_new_json(&arguments.output, &receipt)?;
    Ok(receipt)
}

fn authenticate_session_journal_identity(
    journal: &JournalReceipt,
    expected_schema: &str,
    label: &str,
) -> Result<()> {
    if journal.schema != expected_schema
        || journal.completion_pointer != "/status"
        || journal.completion_value != "finalized"
    {
        return Err(Error::new(format!(
            "{label} journal does not bind its exact top-level finalized status"
        )));
    }
    Ok(())
}

fn authenticate_activity_session(
    evidence_root: &Path,
    relative_path: &str,
    journals: &[JournalReceipt],
    expected_genesis_hash: &str,
) -> Result<Vec<String>> {
    let path = resolve_relative_evidence(evidence_root, relative_path, "activity session")?;
    let bytes = bounded_read(&path, "activity session")?;
    let session: ActivitySession = serde_json::from_value(exact_json(&bytes, "activity session")?)?;
    if session.schema != ACTIVITY_SESSION_SCHEMA_V1
        || session.status != "finalized"
        || session.cluster != "owned-loopback"
        || session.genesis_hash != expected_genesis_hash
        || session.stages.len() != ACTIVITY_STAGES.len()
        || session.completed_stages.len() != ACTIVITY_STAGES.len()
    {
        return Err(Error::new(
            "activity session is not the exact finalized owned-loopback lifecycle",
        ));
    }
    let journal_rows = journals
        .iter()
        .map(|row| (row.path.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut paths = BTreeSet::new();
    for ((stage, completed), expected) in session
        .stages
        .iter()
        .map(|row| row.stage.as_str())
        .zip(session.completed_stages.iter().map(String::as_str))
        .zip(ACTIVITY_STAGES)
    {
        if stage != expected || completed != expected {
            return Err(Error::new(
                "activity session stages or completedStages are noncanonical",
            ));
        }
    }
    for row in &session.stages {
        canonical_relative(&row.path, "activity session stage path")?;
        if row.completion_value != "finalized" || !paths.insert(row.path.as_str()) {
            return Err(Error::new(
                "activity session stage paths repeat or remain provisional",
            ));
        }
        let journal = journal_rows
            .get(row.path.as_str())
            .ok_or_else(|| Error::new("activity session stage is absent from journal closure"))?;
        if journal.sha256 != row.sha256
            || journal.schema != row.schema
            || journal.completion_pointer != row.completion_pointer
            || journal.completion_value != row.completion_value
        {
            return Err(Error::new(
                "activity session stage differs from the authenticated journal closure",
            ));
        }
        let source_path =
            resolve_relative_evidence(evidence_root, &row.path, "activity session stage")?;
        let source_bytes = bounded_read(&source_path, "activity session stage")?;
        let source = exact_json(&source_bytes, "activity session stage")?;
        if sha256(&source_bytes) != row.sha256
            || source
                .as_object()
                .and_then(|object| object.get("schema"))
                .and_then(Value::as_str)
                != Some(row.schema.as_str())
            || json_pointer(&source, &row.completion_pointer)? != &Value::String("finalized".into())
        {
            return Err(Error::new(
                "activity session stage bytes are substituted, provisional, or partial",
            ));
        }
    }
    let stage_set_sha256 = sha256(&canonical_json_bytes(&serde_json::to_value(
        &session.stages,
    )?)?);
    if session.stage_set_sha256 != stage_set_sha256 {
        return Err(Error::new(
            "activity session stageSetSha256 differs from its exact ordered rows",
        ));
    }
    Ok(session.completed_stages)
}

fn authenticate_manifest(
    manifest: &Value,
    capture_genesis: &str,
) -> Result<(Value, BTreeMap<String, String>)> {
    let object = exact_object_keys(
        manifest,
        &[
            "schema",
            "activityId",
            "cluster",
            "accounts",
            "events",
            "finalAccounts",
            "sourceSetSha256",
        ],
        "activity manifest",
    )?;
    if text_field(object, "schema", "activity manifest")? != MANIFEST_SCHEMA_V1 {
        return Err(Error::new("activity manifest is another schema"));
    }
    exact_lower_hex(
        text_field(object, "sourceSetSha256", "activity manifest")?,
        64,
        "activity source-set SHA-256",
    )?;
    let cluster = object
        .get("cluster")
        .ok_or_else(|| Error::new("activity manifest omitted cluster"))?;
    let cluster_object = exact_object_keys(cluster, &["kind", "genesisHash"], "manifest cluster")?;
    if text_field(cluster_object, "kind", "manifest cluster")? != "owned-loopback"
        || text_field(cluster_object, "genesisHash", "manifest cluster")? != capture_genesis
    {
        return Err(Error::new(
            "activity manifest and capture do not name one owned-loopback cluster",
        ));
    }
    let events = object
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("activity manifest events are not an array"))?;
    if events.len() < MANIFEST_EVENT_KINDS.len() || events.len() > 128 {
        return Err(Error::new(
            "activity manifest event chain is not bounded and complete",
        ));
    }
    let mut paths = BTreeMap::new();
    let mut seen_stages = BTreeSet::new();
    let mut prior_stage = 0;
    let mut source_set = Vec::with_capacity(events.len());
    for (index, event) in events.iter().enumerate() {
        let event = event
            .as_object()
            .ok_or_else(|| Error::new(format!("activity event {index} is not an object")))?;
        let id = text_field(event, "id", "activity event")?;
        let stage = text_field(event, "kind", "activity event")?;
        let stage_index = MANIFEST_EVENT_KINDS
            .iter()
            .position(|expected| *expected == stage)
            .ok_or_else(|| Error::new("activity event has an unknown lifecycle kind"))?;
        if index != 0 && stage_index < prior_stage {
            return Err(Error::new("activity lifecycle event kinds move backward"));
        }
        prior_stage = stage_index;
        seen_stages.insert(stage);
        let path = text_field(event, "sourcePath", "activity event")?;
        canonical_relative(path, "activity event source path")?;
        if !path.is_ascii() {
            return Err(Error::new("activity event source path is not ASCII"));
        }
        let source_sha256 = text_field(event, "sourceSha256", "activity event")?;
        exact_lower_hex(source_sha256, 64, "activity event source SHA-256")?;
        if paths
            .insert(path.to_owned(), source_sha256.to_owned())
            .is_some_and(|prior| prior != source_sha256)
        {
            return Err(Error::new(
                "activity events give one source path multiple byte digests",
            ));
        }
        source_set.push(json!({"event": id, "sha256": source_sha256}));
    }
    if seen_stages != MANIFEST_EVENT_KINDS.into_iter().collect() {
        return Err(Error::new(
            "activity manifest does not cover the exact lifecycle stage set",
        ));
    }
    if sha256(&canonical_json_bytes(&Value::Array(source_set))?)
        != text_field(object, "sourceSetSha256", "activity manifest")?
    {
        return Err(Error::new(
            "activity manifest sourceSetSha256 does not bind its ordered events",
        ));
    }
    Ok((cluster.clone(), paths))
}

fn authenticate_journals(
    evidence_root: &Path,
    descriptors: DescriptorDocument,
) -> Result<(Vec<JournalReceipt>, BTreeMap<String, String>)> {
    if descriptors.schema != DESCRIPTORS_SCHEMA_V1
        || descriptors.journals.len() < DESCRIPTOR_ROLES.len()
        || descriptors.journals.len() > MAX_JOURNALS
    {
        return Err(Error::new(
            "journal descriptors are not the exact bounded lifecycle semantic-owner set",
        ));
    }
    let accepted = DESCRIPTOR_ROLES.into_iter().collect::<BTreeSet<_>>();
    let mut roles = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut semantic_paths = BTreeMap::new();
    let mut journals = Vec::with_capacity(descriptors.journals.len());
    for descriptor in descriptors.journals {
        if !accepted.contains(descriptor.semantic_role.as_str()) {
            return Err(Error::new(
                "journal descriptors invent a lifecycle semantic role",
            ));
        }
        let singleton = matches!(
            descriptor.semantic_role.as_str(),
            "activity-session" | "chaos-session"
        );
        if singleton && roles.contains(&descriptor.semantic_role) {
            return Err(Error::new(
                "journal descriptors repeat an aggregate lifecycle session role",
            ));
        }
        roles.insert(descriptor.semantic_role.clone());
        if descriptor.schema.is_empty()
            || descriptor.schema.len() > 160
            || !descriptor.schema.is_ascii()
            || !descriptor.path.is_ascii()
            || !descriptor.completion_pointer.is_ascii()
        {
            return Err(Error::new(
                "journal descriptor strings are empty, unbounded, or non-ASCII",
            ));
        }
        let path = resolve_relative_evidence(evidence_root, &descriptor.path, "journal")?;
        if !paths.insert(descriptor.path.clone()) {
            return Err(Error::new("journal descriptors repeat a journal path"));
        }
        let bytes = bounded_read(&path, "lifecycle journal")?;
        let value = exact_json(&bytes, "lifecycle journal")?;
        let source_schema = value
            .as_object()
            .and_then(|object| object.get("schema"))
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("lifecycle journal omitted its schema"))?;
        if source_schema != descriptor.schema
            || json_pointer(&value, &descriptor.completion_pointer)?
                != &Value::String("finalized".into())
        {
            return Err(Error::new(format!(
                "journal {} is substituted, provisional, or partial",
                descriptor.path
            )));
        }
        if singleton {
            semantic_paths.insert(descriptor.semantic_role, descriptor.path.clone());
        }
        journals.push(JournalReceipt {
            path: descriptor.path,
            sha256: sha256(&bytes),
            schema: descriptor.schema,
            completion_pointer: descriptor.completion_pointer,
            completion_value: "finalized".into(),
        });
    }
    if roles != accepted.into_iter().map(str::to_owned).collect() {
        return Err(Error::new(
            "journal descriptors omit participant, Direct, Pyth, resolution, payout, retirement, or session ownership",
        ));
    }
    journals.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((journals, semantic_paths))
}

fn authenticate_chaos(path: &Path) -> Result<ChaosSession> {
    let bytes = bounded_read(path, "chaos session")?;
    let chaos: ChaosSession = serde_json::from_value(exact_json(&bytes, "chaos session")?)?;
    if chaos.schema != CHAOS_SESSION_SCHEMA_V1
        || chaos.status != "finalized"
        || chaos.stages.len() != CHAOS_STAGES.len()
    {
        return Err(Error::new(
            "chaos session is not one finalized bounded session",
        ));
    }
    for (row, expected) in chaos.stages.iter().zip(CHAOS_STAGES) {
        exact_lower_hex(&row.intent_sha256, 64, "chaos intent SHA-256")?;
        if row.stage != expected || row.status != "finalized" {
            return Err(Error::new(
                "chaos session is not the exact ordered eight-stage sequence",
            ));
        }
    }
    Ok(chaos)
}

fn authenticate_programs(
    plan: &SuccessorPlan,
    capture: &CaptureV1,
    retained_authority: Pubkey,
) -> Result<Vec<ProgramReceipt>> {
    let checked = plan
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("checked local mutable plan evidence is absent"))?;
    if checked.roles.len() != crate::upgrade::CHECKED_ROLE_ORDER_V1.len() {
        return Err(Error::new(
            "checked plan omitted the exact seven program roles",
        ));
    }
    let mut programs = Vec::with_capacity(PROGRAM_ROLES.len());
    let mut closure = BTreeSet::new();
    let retained_authority_text = retained_authority.to_string();
    for (pin, expected_role) in checked
        .roles
        .iter()
        .zip(crate::upgrade::CHECKED_ROLE_ORDER_V1)
    {
        if pin.role != expected_role {
            return Err(Error::new("checked plan program roles are noncanonical"));
        }
        let row = captured_loader_pair(
            capture,
            expected_role,
            &pin.program_id,
            Some(&pin.programdata_id),
            DeploymentSlotPolicyV1::Nonzero,
        )?;
        if row.deployment_slot != pin.deployment_slot.to_string()
            || row.elf_sha256 != pin.live_elf_sha256
            || row.genesis_program_data_sha256 != pin.programdata_account_sha256
            || row.upgrade_authority.as_deref() != Some(checked.retained_upgrade_authority.as_str())
            || row.upgrade_authority.as_deref() != Some(retained_authority_text.as_str())
        {
            return Err(Error::new(format!(
                "captured {expected_role} Loader bytes differ from the authenticated plan"
            )));
        }
        closure.insert(row.program_id.clone());
        closure.insert(row.program_data_address.clone());
        programs.push(row);
    }
    for (role, program_id) in [
        ("pyth-receiver", PYTH_RECEIVER_PROGRAM_ID),
        ("pyth-router", PYTH_ROUTER_PROGRAM_ID),
    ] {
        let row = captured_loader_pair(
            capture,
            role,
            program_id,
            None,
            DeploymentSlotPolicyV1::ExactZeroImmutable,
        )?;
        if row.upgrade_authority.is_some() {
            return Err(Error::new(format!(
                "captured {role} is mutable; provider programs must be installed with no authority"
            )));
        }
        closure.insert(row.program_id.clone());
        closure.insert(row.program_data_address.clone());
        programs.push(row);
    }
    if closure.len() != 18
        || programs
            .iter()
            .map(|row| row.role.as_str())
            .ne(PROGRAM_ROLES)
    {
        return Err(Error::new(
            "finalized capture omitted or aliased the ordered nine-program Loader closure",
        ));
    }
    if capture.loader_addresses_v1()? != closure {
        return Err(Error::new(
            "finalized capture Loader-v3 account set is not the exact 18-account closure",
        ));
    }
    Ok(programs)
}

fn captured_loader_pair(
    capture: &CaptureV1,
    role: &str,
    program_id: &str,
    expected_programdata: Option<&str>,
    slot_policy: DeploymentSlotPolicyV1,
) -> Result<ProgramReceipt> {
    let program = parse_pubkey(program_id, &format!("{role} program ID"))?;
    let expected_programdata = expected_programdata
        .map(|value| parse_pubkey(value, &format!("{role} ProgramData ID")))
        .transpose()?;
    let pair = capture.loader_pair_v1(role, program, expected_programdata, slot_policy)?;
    Ok(ProgramReceipt {
        role: role.into(),
        program_id: program_id.into(),
        program_data_address: pair.program_data.address.to_string(),
        deployment_slot: pair.deployment_slot.to_string(),
        elf_sha256: pair.elf_sha256,
        genesis_program_data_sha256: pair.program_data_sha256,
        upgrade_authority: pair.upgrade_authority.map(|key| key.to_string()),
    })
}

fn authenticate_pyth_update_facts(path: &Path) -> Result<()> {
    let bytes = bounded_read(path, "Pyth facts")?;
    let facts = exact_json(&bytes, "Pyth facts")?;
    let object = exact_object_keys(
        &facts,
        &[
            "format",
            "encodedVaa",
            "updateAccount",
            "postUpdateBodyBase64",
        ],
        "Pyth facts",
    )?;
    if text_field(object, "format", "Pyth facts")? != PYTH_FACTS_SCHEMA_V1 {
        return Err(Error::new("Pyth facts are another schema"));
    }
    let encoded = text_field(object, "encodedVaa", "Pyth facts")?;
    let update = text_field(object, "updateAccount", "Pyth facts")?;
    let body = text_field(object, "postUpdateBodyBase64", "Pyth facts")?;
    let encoded_key = parse_pubkey(encoded, "Pyth EncodedVaa")?;
    let update_key = parse_pubkey(update, "Pyth update account")?;
    let decoded = BASE64
        .decode(body)
        .map_err(|error| Error::new(format!("Pyth PostUpdate body base64: {error}")))?;
    if encoded_key == update_key || decoded.is_empty() || BASE64.encode(&decoded) != body {
        return Err(Error::new(
            "Pyth update facts identities or PostUpdate body are noncanonical",
        ));
    }
    Ok(())
}

fn authenticate_provider_closure_binding(
    closure: &crate::terminal_exterior_pyth::ProviderClosureReceiptV1,
    capture: &CaptureV1,
    capture_path: &Path,
    programs: &[ProgramReceipt],
) -> Result<()> {
    if closure.schema != crate::terminal_exterior_pyth::PROVIDER_CLOSURE_SCHEMA_V1
        || closure.cluster != "owned-loopback"
        || closure.status != "finalized"
        || closure.genesis_hash != capture.genesis_hash
        || closure.finalized_observation_slot != capture.finalized_slot.to_string()
        || closure.finalized_capture.path
            != capture_path
                .to_str()
                .ok_or_else(|| Error::new("capture path is not UTF-8"))?
        || closure.finalized_capture.sha256 != capture.sha256
        || closure.finalized_capture.schema != owned_loopback_capture::SCHEMA_V1
        || closure.finalized_capture.finalized_slot != capture.finalized_slot.to_string()
        || programs.len() != PROGRAM_ROLES.len()
        || closure.provider_programs.len() != 2
    {
        return Err(Error::new(
            "Pyth provider closure does not bind the aggregate finalized capture",
        ));
    }
    for (program, provider) in programs[7..].iter().zip(&closure.provider_programs) {
        if program.role != provider.role
            || program.program_id != provider.program_id
            || program.program_data_address != provider.program_data_address
            || program.deployment_slot != provider.deployment_slot
            || program.elf_sha256 != provider.elf_sha256
            || program.genesis_program_data_sha256 != provider.genesis_program_data_sha256
            || program.upgrade_authority != provider.upgrade_authority
        {
            return Err(Error::new(
                "Pyth provider closure rows differ from the aggregate Loader capture",
            ));
        }
    }
    Ok(())
}

fn set_once<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(format!("{flag} was repeated")));
    }
    Ok(())
}

fn required<T>(value: Option<T>, flag: &str) -> Result<T> {
    value.ok_or_else(|| Error::new(format!("required {flag} is absent")))
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    let metadata = fs::symlink_metadata(path)?;
    let canonical = fs::canonicalize(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || canonical != path {
        return Err(Error::new(format!(
            "{label} must be one canonical non-symlink directory"
        )));
    }
    Ok(canonical)
}

fn canonical_regular(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    let metadata = fs::symlink_metadata(path)?;
    let canonical = fs::canonicalize(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || canonical != path {
        return Err(Error::new(format!(
            "{label} must be one canonical regular non-symlink file"
        )));
    }
    Ok(canonical)
}

fn canonical_relative(value: &str, label: &str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || value.contains('\\')
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || path.to_string_lossy() != value
    {
        return Err(Error::new(format!(
            "{label} is not a canonical relative path"
        )));
    }
    Ok(())
}

fn resolve_relative_evidence(root: &Path, relative: &str, label: &str) -> Result<PathBuf> {
    canonical_relative(relative, label)?;
    let mut current = root.to_owned();
    for component in Path::new(relative).components() {
        let Component::Normal(name) = component else {
            return Err(Error::new(format!("{label} is not canonical")));
        };
        current.push(name);
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new(format!("{label} traverses a symlink")));
        }
    }
    if !fs::symlink_metadata(&current)?.is_file() || fs::canonicalize(&current)? != current {
        return Err(Error::new(format!(
            "{label} is not one canonical regular evidence file"
        )));
    }
    Ok(current)
}

fn relative_evidence_path(root: &Path, path: &Path, label: &str) -> Result<String> {
    let canonical = canonical_regular(path, label)?;
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| Error::new(format!("{label} is outside the evidence root")))?
        .to_str()
        .ok_or_else(|| Error::new(format!("{label} relative path is not UTF-8")))?
        .to_owned();
    canonical_relative(&relative, label)?;
    let resolved = resolve_relative_evidence(root, &relative, label)?;
    if resolved != canonical {
        return Err(Error::new(format!("{label} relative projection changed")));
    }
    Ok(relative)
}

fn bounded_read(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > MAX_INPUT_BYTES {
        return Err(Error::new(format!("{label} is empty or exceeds 32 MiB")));
    }
    Ok(fs::read(path)?)
}

fn exact_json(bytes: &[u8], label: &str) -> Result<Value> {
    crate::rpc::parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn exact_object_keys<'a>(
    value: &'a Value,
    expected: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new(format!("{label} is not an object")))?;
    let observed = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if observed != expected {
        return Err(Error::new(format!("{label} has unknown or omitted fields")));
    }
    Ok(object)
}

fn text_field<'a>(object: &'a Map<String, Value>, field: &str, label: &str) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{label} {field} is not text")))
}

fn exact_lower_hex(value: &str, length: usize, label: &str) -> Result<()> {
    if value.len() != length
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(Error::new(format!(
            "{label} is not canonical lowercase hex"
        )));
    }
    Ok(())
}

fn canonical_decimal(value: &str, label: &str) -> Result<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::new(format!("{label} is not canonical decimal")));
    }
    value
        .parse()
        .map_err(|_| Error::new(format!("{label} overflows u64")))
}

fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    value
        .parse()
        .map_err(|_| Error::new(format!("{label} is not a 32-byte Solana address")))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn json_pointer<'a>(value: &'a Value, pointer: &str) -> Result<&'a Value> {
    if !pointer.starts_with('/') {
        return Err(Error::new("completionPointer is not canonical RFC6901"));
    }
    let mut current = value;
    for raw in pointer[1..].split('/') {
        let mut index = 0;
        while let Some(offset) = raw[index..].find('~') {
            index += offset;
            if raw
                .as_bytes()
                .get(index + 1)
                .is_none_or(|byte| !matches!(byte, b'0' | b'1'))
            {
                return Err(Error::new(
                    "completionPointer has an invalid RFC6901 escape",
                ));
            }
            index += 2;
        }
        let part = raw.replace("~1", "/").replace("~0", "~");
        current = match current {
            Value::Object(object) => object
                .get(&part)
                .ok_or_else(|| Error::new("completionPointer is absent"))?,
            Value::Array(array)
                if part.bytes().all(|byte| byte.is_ascii_digit())
                    && (part == "0" || !part.starts_with('0')) =>
            {
                let index: usize = part
                    .parse()
                    .map_err(|_| Error::new("completionPointer array index overflows"))?;
                array
                    .get(index)
                    .ok_or_else(|| Error::new("completionPointer is absent"))?
            }
            _ => return Err(Error::new("completionPointer is absent")),
        };
    }
    Ok(current)
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>> {
    fn append(value: &Value, output: &mut String) -> Result<()> {
        match value {
            Value::Null => output.push_str("null"),
            Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
            Value::Number(value) => output.push_str(&value.to_string()),
            Value::String(value) => output.push_str(&serde_json::to_string(value)?),
            Value::Array(values) => {
                output.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.push(',');
                    }
                    append(value, output)?;
                }
                output.push(']');
            }
            Value::Object(values) => {
                output.push('{');
                let mut keys = values.keys().collect::<Vec<_>>();
                keys.sort();
                for (index, key) in keys.into_iter().enumerate() {
                    if index != 0 {
                        output.push(',');
                    }
                    output.push_str(&serde_json::to_string(key)?);
                    output.push(':');
                    append(&values[key], output)?;
                }
                output.push('}');
            }
        }
        Ok(())
    }
    let mut output = String::new();
    append(value, &mut output)?;
    output.push('\n');
    Ok(output.into_bytes())
}

fn write_new_json(path: &Path, value: &Value) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new("receipt output must be absolute"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("receipt output has no parent"))?;
    canonical_directory(parent, "receipt output parent")?;
    let bytes = canonical_json_bytes(value)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6901_requires_terminal_finalized_value() {
        let value = json!({"rows": [{"phase": "finalized"}]});
        assert_eq!(
            json_pointer(&value, "/rows/0/phase").expect("canonical pointer"),
            "finalized"
        );
        assert!(json_pointer(&value, "/rows/00/phase").is_err());
        assert!(json_pointer(&value, "/rows/~2/phase").is_err());
        assert!(json_pointer(&value, "rows/0/phase").is_err());
    }

    #[test]
    fn canonical_rows_match_python_sorted_compact_shape() {
        let value = json!([{
            "path": "z.json",
            "sha256": "aa",
            "schema": "owner-v1",
            "completionPointer": "/phase",
            "completionValue": "finalized"
        }]);
        assert_eq!(
            String::from_utf8(canonical_json_bytes(&value).expect("canonical JSON"))
                .expect("UTF-8"),
            "[{\"completionPointer\":\"/phase\",\"completionValue\":\"finalized\",\"path\":\"z.json\",\"schema\":\"owner-v1\",\"sha256\":\"aa\"}]\n"
        );
    }

    #[test]
    fn chaos_vocabularies_remain_distinct() {
        assert_eq!(ACTIVITY_STAGES[4], "direct");
        assert_eq!(CHAOS_STAGES[4], "hot");
        assert_eq!(ACTIVITY_STAGES[7], "retirement");
        assert_eq!(CHAOS_STAGES[7], "retire");
        assert_eq!(
            MANIFEST_EVENT_KINDS,
            [
                "founding",
                "participant",
                "direct",
                "resolution",
                "payout",
                "retirement",
            ]
        );
        assert!(!MANIFEST_EVENT_KINDS.contains(&"alt"));
        assert!(!MANIFEST_EVENT_KINDS.contains(&"seal"));
    }

    #[test]
    fn chaos_session_requires_exact_schema_and_vocabulary() {
        let path = fs::canonicalize(std::env::temp_dir())
            .expect("canonical temp")
            .join(format!(
                "dclutch-private-lifecycle-chaos-{}.json",
                std::process::id()
            ));
        let mut session = json!({
            "schema": CHAOS_SESSION_SCHEMA_V1,
            "status": "finalized",
            "stages": CHAOS_STAGES
                .iter()
                .map(|stage| json!({
                    "stage": stage,
                    "status": "finalized",
                    "intentSha256": "11".repeat(32),
                }))
                .collect::<Vec<_>>(),
        });
        fs::write(
            &path,
            serde_json::to_vec(&session).expect("chaos session JSON"),
        )
        .expect("write chaos session");
        authenticate_chaos(&path).expect("exact chaos session");
        session["schema"] = Value::String("dclutch-lifecycle-chaos-summary-v1".into());
        fs::write(
            &path,
            serde_json::to_vec(&session).expect("substituted chaos JSON"),
        )
        .expect("write substituted chaos session");
        assert!(authenticate_chaos(&path).is_err());
        fs::remove_file(path).expect("remove chaos session");

        let mut journal = JournalReceipt {
            path: "chaos.json".into(),
            sha256: "11".repeat(32),
            schema: CHAOS_SESSION_SCHEMA_V1.into(),
            completion_pointer: "/status".into(),
            completion_value: "finalized".into(),
        };
        authenticate_session_journal_identity(&journal, CHAOS_SESSION_SCHEMA_V1, "chaos session")
            .expect("exact top-level status");
        journal.completion_pointer = "/stages/0/status".into();
        assert!(
            authenticate_session_journal_identity(
                &journal,
                CHAOS_SESSION_SCHEMA_V1,
                "chaos session",
            )
            .is_err()
        );
    }

    #[test]
    fn activity_session_reopens_exact_eight_stage_closure() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = fs::canonicalize(std::env::temp_dir())
            .expect("canonical temp")
            .join(format!(
                "dclutch-private-lifecycle-session-{}-{nonce}",
                std::process::id()
            ));
        fs::create_dir(&root).expect("create evidence root");
        let mut stages = Vec::new();
        let mut journals = Vec::new();
        for stage in ACTIVITY_STAGES {
            let relative = format!("{stage}.json");
            let source_path = root.join(&relative);
            let source =
                json!({"schema": format!("dclutch-test-{stage}-v1"), "status": "finalized"});
            let source_bytes = serde_json::to_vec(&source).expect("source JSON");
            fs::write(&source_path, &source_bytes).expect("write stage source");
            let row = ActivitySessionStage {
                stage: stage.into(),
                path: relative.clone(),
                sha256: sha256(&source_bytes),
                schema: format!("dclutch-test-{stage}-v1"),
                completion_pointer: "/status".into(),
                completion_value: "finalized".into(),
            };
            journals.push(JournalReceipt {
                path: relative,
                sha256: row.sha256.clone(),
                schema: row.schema.clone(),
                completion_pointer: row.completion_pointer.clone(),
                completion_value: row.completion_value.clone(),
            });
            stages.push(row);
        }
        let session_path = root.join("session.json");
        let stage_set_sha256 = sha256(
            &canonical_json_bytes(&serde_json::to_value(&stages).expect("stages"))
                .expect("canonical stages"),
        );
        let mut session = json!({
            "schema": ACTIVITY_SESSION_SCHEMA_V1,
            "status": "finalized",
            "cluster": "owned-loopback",
            "genesisHash": "owned-loopback-genesis",
            "stages": stages,
            "completedStages": ACTIVITY_STAGES,
            "stageSetSha256": stage_set_sha256,
        });
        fs::write(
            &session_path,
            serde_json::to_vec(&session).expect("session JSON"),
        )
        .expect("write session");
        assert_eq!(
            authenticate_activity_session(
                &root,
                "session.json",
                &journals,
                "owned-loopback-genesis",
            )
            .expect("exact session"),
            ACTIVITY_STAGES
        );

        session["completedStages"]
            .as_array_mut()
            .expect("completed stages")
            .pop();
        fs::write(
            &session_path,
            serde_json::to_vec(&session).expect("partial session JSON"),
        )
        .expect("write partial session");
        assert!(
            authenticate_activity_session(
                &root,
                "session.json",
                &journals,
                "owned-loopback-genesis",
            )
            .is_err()
        );
        fs::remove_dir_all(root).expect("remove evidence root");
    }

    #[test]
    fn pyth_update_facts_remain_independent_from_provider_closure() {
        let path = fs::canonicalize(std::env::temp_dir())
            .expect("canonical temp")
            .join(format!(
                "dclutch-private-lifecycle-pyth-facts-{}.json",
                std::process::id()
            ));
        let facts = json!({
            "format": PYTH_FACTS_SCHEMA_V1,
            "encodedVaa": Pubkey::new_unique().to_string(),
            "updateAccount": Pubkey::new_unique().to_string(),
            "postUpdateBodyBase64": BASE64.encode([1_u8]),
        });
        fs::write(&path, serde_json::to_vec(&facts).expect("facts JSON")).expect("write facts");
        authenticate_pyth_update_facts(&path).expect("update facts own only update fixture");
        fs::remove_file(path).expect("remove facts");
    }
}
