//! Producers for the public Direct Hot route evidence the SDK reads.
//!
//! Two key-free, read-only devnet commands live here.
//!
//! `devnet-checked-execution-release-v1` materializes the 1592-byte
//! `dclutch-checked-multiprogram-v1` file for the LIVE activated release set:
//! the five embedded `ArtifactReleaseV1` records are read verbatim out of the
//! finalized Registry activation cache (the chain is their author), and the
//! five checked-release identities come from the sha-pinned checked-release
//! manifests the deployment sealed. Nothing here invents a release fact: the
//! bytes are assembled and then handed to
//! `dclutch_release_tool::CheckedExecutionReleaseSetV1::decode`, whose
//! revalidation is the sole admission gate.
//!
//! `devnet-direct-hot-route-manifest-v3` emits the one public JSON envelope
//! `dclutch-direct-hot-route-manifest-v3` that `@dclutch/sdk`'s
//! `inspectDirectHotRouteManifestJsonV3` admits. The route facts are not
//! restated: they are projected from the SAME `DirectTradePlanningV1` the
//! trade executor plans with (`direct_trade::collect_direct_trade_planning_v1`
//! over one finalized snapshot), after the session's journal proves the frozen
//! lookup table exists. The embedded 2360-byte checked-infrastructure evidence
//! is assembled from the chain-served profile and Registry/Rent artifact
//! records plus the sealed checked identities, then revalidated by
//! `dclutch_release_tool::CheckedInfrastructureV1::decode`.
//!
//! Every refusal carries a stable bracketed code so a caller can pin the exact
//! wall: `[route-manifest/<name>]` / `[checked-execution/<name>]`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_registry::{ARTIFACT_RELEASE_BYTES_V1, ActivatedExecutionReleaseSetV1};
use dclutch_registry::release_set::{EXECUTION_RELEASE_SET_BYTES_V1, ExecutionRoleV1};
use dclutch_release_tool::{
    CHECKED_INFRASTRUCTURE_BYTES_V1, CHECKED_INFRASTRUCTURE_COMPONENTS_V1,
    CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1, CHECKED_INFRASTRUCTURE_MAGIC_V1,
    CHECKED_INFRASTRUCTURE_SCHEMA_V2, CHECKED_MULTIPROGRAM_BYTES_V1,
    CHECKED_MULTIPROGRAM_HEADER_BYTES_V1, CHECKED_MULTIPROGRAM_MAGIC_V1,
    CHECKED_MULTIPROGRAM_SCHEMA_V1, CheckedExecutionReleaseSetV1, CheckedInfrastructureV1,
    CheckedReleaseV1,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest as _, Sha256};

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_GENESIS_HASH, ExpectedClusterV1},
    direct_trade,
    model::{ProgramPin, SuccessorPlan},
    plan::pubkey,
    rpc::{Rpc, WritePolicyV1},
};

pub(crate) const CHECKED_EXECUTION_RELEASE_COMMAND_V1: &str = "devnet-checked-execution-release-v1";
pub(crate) const HOT_ROUTE_MANIFEST_COMMAND_V3: &str = "devnet-direct-hot-route-manifest-v3";

/// The only public JSON envelope admitted for one Direct InlineOrdinary route.
///
/// Format owner: `packages/dclutch-sdk/lib/directHotRouteManifest.ts`. This
/// constant and the role labels below are that reader's transport vocabulary;
/// the account MEANING stays owned by the generated Hot V3 coordinates.
const DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3: &str = "dclutch-direct-hot-route-manifest-v3";

/// Reader labels for the exact 39-coordinate fixed frame, by Hot V3 index.
///
/// Owner: `DIRECT_HOT_FIXED_ROLE_LABELS_V3` in the SDK reader. The reader
/// refuses any row whose label differs from this exact index-aligned text, so
/// the emitter speaks the reader's vocabulary verbatim.
const DIRECT_HOT_FIXED_ROLE_LABELS_V3: [&str; 39] = [
    "Market",
    "Direct root",
    "Manifest raw",
    "Manifest staging",
    "ProgramSet raw",
    "ProgramSet staging",
    "Descriptor raw",
    "Descriptor staging",
    "Config raw",
    "Config staging",
    "AccountProfile raw",
    "AccountProfile staging",
    "RequestProfile raw",
    "RequestProfile staging",
    "Transition raw",
    "Transition staging",
    "Effect raw",
    "Effect staging",
    "Lifecycle raw",
    "Lifecycle staging",
    "Strategy raw",
    "Strategy staging",
    "Activation cache",
    "Core program",
    "Core ProgramData",
    "Trading program",
    "Trading ProgramData",
    "Registry program",
    "Rent sysvar",
    "Instructions sysvar",
    "Product raw",
    "Product staging",
    "Result domain raw",
    "Result domain staging",
    "Portfolio raw",
    "Portfolio staging",
    "Product basis raw",
    "Product basis staging",
    "Capability seal",
];

/// The physical runtime prefix aliased onto fixed rows before the 39-row tail.
///
/// Owner: `assemble_direct_inline_ordinary_route_v3` in
/// `dclutch_operator::direct_inline_route_v3`, which pins physical runtime
/// rows 0..5 to the root/config/product/portfolio/basis raw fixed rows; the
/// SDK reader's `runtimeAccounts` begin after them.
const DIRECT_RUNTIME_FIXED_PREFIX_ROWS_V3: usize = 5;

const EXECUTION_ROLE_ORDER_V1: [(ExecutionRoleV1, &str); 5] = [
    (ExecutionRoleV1::Core, "core"),
    (ExecutionRoleV1::Claims, "claims"),
    (ExecutionRoleV1::Trading, "trading"),
    (ExecutionRoleV1::Resolution, "resolution"),
    (ExecutionRoleV1::Custody, "custody"),
];

fn refusal(code: &str, reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: [{code}] {}", reason.as_ref()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_text(&hasher.finalize())
}

fn hex_text(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn exact_hex64(value: &str, label: &str) -> Result<String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(refusal(
            "input/expected-sha256",
            format!("{label} expectation is not exact lowercase SHA-256 hex"),
        ));
    }
    Ok(value.to_string())
}

/// Read one absolute regular file and require its exact pinned digest.
fn pinned_bytes(path: &Path, expected_sha256: &str, label: &str) -> Result<Vec<u8>> {
    if !path.is_absolute() {
        return Err(refusal(
            "input/relative-path",
            format!("{label} path must be absolute: {}", path.display()),
        ));
    }
    let bytes = fs::read(path).map_err(|error| {
        refusal(
            "input/unreadable",
            format!("{label} {}: {error}", path.display()),
        )
    })?;
    let observed = sha256_hex(&bytes);
    if observed != expected_sha256 {
        return Err(refusal(
            "input/sha256-mismatch",
            format!(
                "{label} {} hashes to {observed}, expected {expected_sha256}",
                path.display()
            ),
        ));
    }
    Ok(bytes)
}

fn write_create_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if !path.is_absolute() {
        return Err(refusal(
            "output/relative-path",
            format!("output path must be absolute: {}", path.display()),
        ));
    }
    if path.exists() {
        return Err(refusal(
            "output/exists",
            format!("refusing to overwrite existing output {}", path.display()),
        ));
    }
    fs::write(path, bytes)
        .map_err(|error| refusal("output/unwritable", format!("{}: {error}", path.display())))
}

fn stdout_json(value: &impl Serialize) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|error| Error::new(format!("report serialization: {error}")))?;
    text.push('\n');
    print!("{text}");
    Ok(())
}

// ------------------------------------------------------------- argument walk

struct FlagWalkV1 {
    pairs: Vec<(String, String)>,
}

impl FlagWalkV1 {
    fn parse(arguments: Vec<String>, usage: &str) -> Result<Self> {
        let mut pairs = Vec::new();
        let mut iterator = arguments.into_iter();
        while let Some(flag) = iterator.next() {
            if !flag.starts_with("--") {
                return Err(refusal(
                    "input/unknown-argument",
                    format!("unexpected argument {flag}; usage: {usage}"),
                ));
            }
            let value = iterator.next().ok_or_else(|| {
                refusal(
                    "input/missing-value",
                    format!("{flag} requires a value; usage: {usage}"),
                )
            })?;
            pairs.push((flag, value));
        }
        Ok(Self { pairs })
    }

    fn take(&mut self, flag: &str, usage: &str) -> Result<String> {
        let mut found = Vec::new();
        self.pairs.retain(|(name, value)| {
            if name == flag {
                found.push(value.clone());
                false
            } else {
                true
            }
        });
        match found.len() {
            1 => Ok(found.remove(0)),
            0 => Err(refusal(
                "input/missing-flag",
                format!("{flag} is required; usage: {usage}"),
            )),
            _ => Err(refusal(
                "input/repeated-flag",
                format!("{flag} may appear exactly once"),
            )),
        }
    }

    fn finish(self) -> Result<()> {
        if let Some((flag, _)) = self.pairs.first() {
            return Err(refusal(
                "input/unknown-flag",
                format!("unknown flag {flag}"),
            ));
        }
        Ok(())
    }
}

fn devnet_rpc(walk: &mut FlagWalkV1, usage: &str) -> Result<Rpc> {
    let rpc_url = walk.take("--rpc-url", usage)?;
    let acknowledgment = walk.take("--i-mean-devnet", usage)?;
    if acknowledgment != DEVNET_GENESIS_HASH {
        return Err(refusal(
            "input/devnet-acknowledgment",
            format!("--i-mean-devnet must be exactly {DEVNET_GENESIS_HASH}"),
        ));
    }
    let origin = ClusterOriginV1::parse(&rpc_url, Some(DEVNET_GENESIS_HASH))?;
    ExpectedClusterV1::Devnet.authenticate(&origin)?;
    Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)
}

fn pinned_named(
    walk: &mut FlagWalkV1,
    flag: &str,
    expected_flag: &str,
    label: &str,
    usage: &str,
) -> Result<(PathBuf, Vec<u8>)> {
    let path = PathBuf::from(walk.take(flag, usage)?);
    let expected = exact_hex64(&walk.take(expected_flag, usage)?, label)?;
    let bytes = pinned_bytes(&path, &expected, label)?;
    Ok((path, bytes))
}

// ------------------------------------------- checked-release evidence joins

/// Admit one sealed checked-release manifest for one deployed role.
///
/// The checked release is deliberately deployment-agnostic source evidence;
/// the joins that bind it to THIS deployment are the plan's sealed source
/// revision, source-tree digest, and exact raw candidate ELF digest.
fn admit_checked_release(
    bytes: &[u8],
    role: &str,
    pin_elf_sha256: &str,
    source_revision: &str,
    source_tree_sha256: &str,
) -> Result<CheckedReleaseV1> {
    let checked = CheckedReleaseV1::decode(bytes).map_err(|error| {
        refusal(
            "checked-release/undecodable",
            format!("{role} checked release refused: {error:?}"),
        )
    })?;
    if checked.source_revision() != source_revision {
        return Err(refusal(
            "checked-release/source-revision",
            format!(
                "{role} checked release binds source revision {}, the sealed deployment binds {source_revision}",
                checked.source_revision()
            ),
        ));
    }
    if hex_text(&checked.source_digest()) != source_tree_sha256 {
        return Err(refusal(
            "checked-release/source-tree",
            format!("{role} checked release binds another source tree"),
        ));
    }
    let artifact = hex_text(&checked.artifact_digest());
    if artifact != pin_elf_sha256 {
        return Err(refusal(
            "checked-release/elf-digest",
            format!(
                "{role} checked release binds ELF {artifact}, the sealed candidate is {pin_elf_sha256}"
            ),
        ));
    }
    Ok(checked)
}

fn role_pin<'plan>(plan: &'plan SuccessorPlan, role: &str) -> Result<&'plan ProgramPin> {
    Ok(match role {
        "core" => &plan.core,
        "claims" => &plan.claims,
        "trading" => &plan.trading,
        "resolution" => &plan.resolution,
        "custody" => &plan.custody,
        "registry" => &plan.registry,
        "rent" => &plan.rent_credit,
        other => {
            return Err(refusal(
                "checked-release/unknown-role",
                format!("no plan pin for role {other}"),
            ));
        }
    })
}

/// Assemble and revalidate the live five-role checked multiprogram bytes.
fn assemble_checked_execution_release_v1(
    plan: &SuccessorPlan,
    activation_cache: &[u8],
    checked_by_role: &[(&str, &[u8]); 5],
) -> Result<(Vec<u8>, CheckedExecutionReleaseSetV1)> {
    let set_pin = plan.checked_upgrade_set.as_ref().ok_or_else(|| {
        refusal(
            "checked-execution/plan-unsealed",
            "devnet plan omitted its authenticated permanent checked deployment set",
        )
    })?;
    let activated = ActivatedExecutionReleaseSetV1::decode(activation_cache).map_err(|error| {
        refusal(
            "checked-execution/activation-cache",
            format!("Registry activation cache refused: {error:?}"),
        )
    })?;
    if hex_text(activated.execution_release_set_id().as_bytes()) != plan.release_set_id {
        return Err(refusal(
            "checked-execution/release-set",
            format!(
                "activation cache selects release set {}, the plan pins {}",
                hex_text(activated.execution_release_set_id().as_bytes()),
                plan.release_set_id
            ),
        ));
    }
    let release_set = activated.release_set_projection().map_err(|error| {
        refusal(
            "checked-execution/release-set-projection",
            format!("cached release-set projection refused: {error:?}"),
        )
    })?;

    let mut bytes = vec![0_u8; CHECKED_MULTIPROGRAM_BYTES_V1];
    bytes[..8].copy_from_slice(&CHECKED_MULTIPROGRAM_MAGIC_V1);
    bytes[8..10].copy_from_slice(&CHECKED_MULTIPROGRAM_SCHEMA_V1.to_le_bytes());
    bytes[10..12].copy_from_slice(&5_u16.to_le_bytes());
    let set_offset = CHECKED_MULTIPROGRAM_HEADER_BYTES_V1;
    bytes[set_offset..set_offset + EXECUTION_RELEASE_SET_BYTES_V1]
        .copy_from_slice(&release_set.to_bytes());

    for (index, (role_value, role_name)) in EXECUTION_ROLE_ORDER_V1.iter().enumerate() {
        let (supplied_name, checked_bytes) = checked_by_role[index];
        if supplied_name != *role_name {
            return Err(refusal(
                "checked-execution/role-order",
                format!("expected {role_name} at position {index}, received {supplied_name}"),
            ));
        }
        let pin = role_pin(plan, role_name)?;
        let checked = admit_checked_release(
            checked_bytes,
            role_name,
            &pin.checked_candidate_elf_sha256,
            &set_pin.source_revision,
            &set_pin.source_tree_sha256,
        )?;
        let artifact = activated.role(*role_value).release();
        let artifact_bytes = artifact.to_bytes();
        let plan_program = pubkey(&pin.program_id)?;
        if artifact.program().to_bytes() != plan_program.to_bytes() {
            // The cache activated another program than the plan pinned, which
            // no downstream consumer would accept.
            return Err(refusal(
                "checked-execution/role-program",
                format!(
                    "{role_name} activated artifact binds another program than the plan pin {}",
                    pin.program_id
                ),
            ));
        }
        let checked_id = checked.checked_release_id().map_err(|error| {
            refusal(
                "checked-release/identity",
                format!("{role_name} checked release identity refused: {error:?}"),
            )
        })?;
        let offset = CHECKED_MULTIPROGRAM_HEADER_BYTES_V1
            + EXECUTION_RELEASE_SET_BYTES_V1
            + index * (ARTIFACT_RELEASE_BYTES_V1 + 32);
        bytes[offset..offset + ARTIFACT_RELEASE_BYTES_V1].copy_from_slice(&artifact_bytes);
        bytes[offset + ARTIFACT_RELEASE_BYTES_V1..offset + ARTIFACT_RELEASE_BYTES_V1 + 32]
            .copy_from_slice(checked_id.as_bytes());
    }

    let decoded = CheckedExecutionReleaseSetV1::decode(&bytes).map_err(|error| {
        refusal(
            "checked-execution/revalidation",
            format!("assembled multiprogram refused by its own decoder: {error:?}"),
        )
    })?;
    let decoded_set_id = decoded.execution_release_set_id().map_err(|error| {
        refusal(
            "checked-execution/identity",
            format!("assembled multiprogram identity refused: {error:?}"),
        )
    })?;
    if hex_text(decoded_set_id.as_bytes()) != plan.release_set_id {
        return Err(refusal(
            "checked-execution/release-set",
            "assembled multiprogram selects another release set than the plan",
        ));
    }
    Ok((bytes, decoded))
}

pub(crate) fn checked_execution_release_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-checked-execution-release-v1 \
     --rpc-url DEVNET_HTTPS_URL \
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
     --plan ABSOLUTE_JSON --expected-plan-sha256 HEX64 \
     --core-checked ABS --expected-core-checked-sha256 HEX64 \
     --claims-checked ABS --expected-claims-checked-sha256 HEX64 \
     --trading-checked ABS --expected-trading-checked-sha256 HEX64 \
     --resolution-checked ABS --expected-resolution-checked-sha256 HEX64 \
     --custody-checked ABS --expected-custody-checked-sha256 HEX64 \
     --output ABSOLUTE_NEW_FILE"
}

pub(crate) fn run_checked_execution_release(arguments: Vec<String>) -> Result<()> {
    let usage = checked_execution_release_usage();
    let mut walk = FlagWalkV1::parse(arguments, usage)?;
    let mut rpc = devnet_rpc(&mut walk, usage)?;
    let (_, plan_bytes) = pinned_named(
        &mut walk,
        "--plan",
        "--expected-plan-sha256",
        "successor plan",
        usage,
    )?;
    let mut checked_files: Vec<(String, Vec<u8>)> = Vec::new();
    for (_, role) in EXECUTION_ROLE_ORDER_V1 {
        let flag = format!("--{role}-checked");
        let expected_flag = format!("--expected-{role}-checked-sha256");
        let (_, bytes) = pinned_named(
            &mut walk,
            &flag,
            &expected_flag,
            &format!("{role} checked release"),
            usage,
        )?;
        checked_files.push((role.to_string(), bytes));
    }
    let output = PathBuf::from(walk.take("--output", usage)?);
    walk.finish()?;

    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)
        .map_err(|error| Error::new(format!("successor plan: {error}")))?;
    let activation = pubkey(&plan.activation)?;
    let registry_program = pubkey(&plan.registry.program_id)?;
    let cache = rpc.required_account(activation, "Registry activation cache")?;
    if cache.owner != registry_program || cache.executable {
        return Err(refusal(
            "checked-execution/activation-owner",
            "activation cache has the wrong Registry owner or executable flag",
        ));
    }

    let checked_by_role: [(&str, &[u8]); 5] = [
        (&checked_files[0].0, &checked_files[0].1),
        (&checked_files[1].0, &checked_files[1].1),
        (&checked_files[2].0, &checked_files[2].1),
        (&checked_files[3].0, &checked_files[3].1),
        (&checked_files[4].0, &checked_files[4].1),
    ];
    let (bytes, decoded) =
        assemble_checked_execution_release_v1(&plan, &cache.data, &checked_by_role)?;
    write_create_new(&output, &bytes)?;

    let checked_id = decoded
        .checked_execution_release_set_id()
        .map_err(|error| Error::new(format!("checked multiprogram identity: {error:?}")))?;
    stdout_json(&json!({
        "schema": "dclutch-devnet-checked-execution-release-report-v1",
        "output": output.display().to_string(),
        "bytes": bytes.len(),
        "sha256": sha256_hex(&bytes),
        "executionReleaseSetId": plan.release_set_id,
        "checkedExecutionReleaseSetId": hex_text(checked_id.as_bytes()),
    }))
}

// ----------------------------------------------- checked infrastructure blob

fn record_account(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    record: &str,
    label: &str,
) -> Result<Vec<u8>> {
    let pair = plan.records.get(record).ok_or_else(|| {
        refusal(
            "route-manifest/plan-record",
            format!("plan omits the {record} record"),
        )
    })?;
    let address = pubkey(&pair.raw)?;
    let account = rpc.required_account(address, label)?;
    if account.owner != pubkey(&plan.registry.program_id)? || account.executable {
        return Err(refusal(
            "route-manifest/record-owner",
            format!("{label} {address} has the wrong Registry owner or executable flag"),
        ));
    }
    if sha256_hex(&account.data) != pair.content_sha256 {
        return Err(refusal(
            "route-manifest/record-content",
            format!("{label} {address} bytes differ from the plan's sealed content digest"),
        ));
    }
    if account.data.len() != ARTIFACT_RELEASE_BYTES_V1 {
        return Err(refusal(
            "route-manifest/record-width",
            format!("{label} {address} is not one exact artifact release record"),
        ));
    }
    Ok(account.data)
}

/// Assemble and revalidate the 2360-byte user-supplied infrastructure blob.
#[allow(clippy::too_many_arguments)]
fn assemble_checked_infrastructure_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    multiprogram_bytes: &[u8],
    registry_checked_bytes: &[u8],
    rent_checked_bytes: &[u8],
) -> Result<(Vec<u8>, CheckedInfrastructureV1)> {
    let set_pin = plan.checked_upgrade_set.as_ref().ok_or_else(|| {
        refusal(
            "route-manifest/plan-unsealed",
            "devnet plan omitted its authenticated permanent checked deployment set",
        )
    })?;
    if multiprogram_bytes.len() != CHECKED_MULTIPROGRAM_BYTES_V1 {
        return Err(refusal(
            "route-manifest/multiprogram-width",
            "checked execution release has the wrong exact width",
        ));
    }

    let profile_address = pubkey(&plan.infrastructure_profile.address)?;
    let profile = rpc.required_account(profile_address, "infrastructure profile")?;
    if sha256_hex(&profile.data) != plan.infrastructure_profile.body_sha256 {
        return Err(refusal(
            "route-manifest/profile-content",
            "infrastructure profile bytes differ from the plan's sealed digest",
        ));
    }

    let registry_record = record_account(
        rpc,
        plan,
        "registry_artifact_release",
        "Registry artifact record",
    )?;
    let rent_record = record_account(rpc, plan, "rent_artifact_release", "Rent artifact record")?;

    let registry_checked = admit_checked_release(
        registry_checked_bytes,
        "registry",
        &plan.registry.checked_candidate_elf_sha256,
        &set_pin.source_revision,
        &set_pin.source_tree_sha256,
    )?;
    let rent_checked = admit_checked_release(
        rent_checked_bytes,
        "rent",
        &plan.rent_credit.checked_candidate_elf_sha256,
        &set_pin.source_revision,
        &set_pin.source_tree_sha256,
    )?;
    let registry_checked_id = registry_checked
        .checked_release_id()
        .map_err(|error| Error::new(format!("registry checked identity: {error:?}")))?;
    let rent_checked_id = rent_checked
        .checked_release_id()
        .map_err(|error| Error::new(format!("rent checked identity: {error:?}")))?;

    let mut bytes = vec![0_u8; CHECKED_INFRASTRUCTURE_BYTES_V1];
    bytes[..8].copy_from_slice(&CHECKED_INFRASTRUCTURE_MAGIC_V1);
    bytes[8..10].copy_from_slice(&CHECKED_INFRASTRUCTURE_SCHEMA_V2.to_le_bytes());
    bytes[10..12].copy_from_slice(&CHECKED_INFRASTRUCTURE_COMPONENTS_V1.to_le_bytes());
    let mut offset = CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1;
    bytes[offset..offset + CHECKED_MULTIPROGRAM_BYTES_V1].copy_from_slice(multiprogram_bytes);
    offset += CHECKED_MULTIPROGRAM_BYTES_V1;
    bytes[offset..offset + profile.data.len()].copy_from_slice(&profile.data);
    offset += profile.data.len();
    bytes[offset..offset + 32].copy_from_slice(&profile_address.to_bytes());
    offset += 32;
    bytes[offset..offset + ARTIFACT_RELEASE_BYTES_V1].copy_from_slice(&registry_record);
    offset += ARTIFACT_RELEASE_BYTES_V1;
    bytes[offset..offset + 32].copy_from_slice(registry_checked_id.as_bytes());
    offset += 32;
    bytes[offset..offset + ARTIFACT_RELEASE_BYTES_V1].copy_from_slice(&rent_record);
    offset += ARTIFACT_RELEASE_BYTES_V1;
    bytes[offset..offset + 32].copy_from_slice(rent_checked_id.as_bytes());
    offset += 32;
    if offset != CHECKED_INFRASTRUCTURE_BYTES_V1 {
        return Err(refusal(
            "route-manifest/blob-assembly",
            "assembled checked infrastructure has the wrong exact width",
        ));
    }

    let decoded = CheckedInfrastructureV1::decode(&bytes).map_err(|error| {
        refusal(
            "route-manifest/blob-revalidation",
            format!("assembled checked infrastructure refused by its own decoder: {error:?}"),
        )
    })?;
    Ok((bytes, decoded))
}

// ----------------------------------------------------------- route manifest

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFixedCoordinateV3 {
    role: &'static str,
    address: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestCoordinateV3 {
    address: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectHotRouteManifestDocumentV3 {
    format: &'static str,
    payer: String,
    fixed_accounts: Vec<ManifestFixedCoordinateV3>,
    strategy_accounts: Vec<ManifestCoordinateV3>,
    runtime_accounts: Vec<ManifestCoordinateV3>,
    lookup_tables: Vec<String>,
    lookup_table_creation_slot: String,
    checked_infrastructure: String,
    checked_infrastructure_sha256: String,
}

/// Mirror the reader's fixed-frame constraints so a refused manifest is
/// unconstructible rather than discovered by a consumer.
fn project_manifest_document_v3(
    planning: &direct_trade::DirectTradePlanningV1,
    blob: &[u8],
) -> Result<DirectHotRouteManifestDocumentV3> {
    let physical = &planning.route.physical;
    if physical.fixed_accounts.len() != DIRECT_HOT_FIXED_ROLE_LABELS_V3.len() {
        return Err(refusal(
            "route-manifest/fixed-count",
            format!(
                "route projects {} fixed accounts; the public envelope carries exactly {}",
                physical.fixed_accounts.len(),
                DIRECT_HOT_FIXED_ROLE_LABELS_V3.len()
            ),
        ));
    }
    if !physical.strategy_accounts.is_empty() {
        return Err(refusal(
            "route-manifest/strategy-nonempty",
            "ordinary Direct routes carry no strategy accounts",
        ));
    }
    let payer = planning.route.payer;
    let mut fixed = Vec::with_capacity(physical.fixed_accounts.len());
    let mut fixed_addresses = std::collections::BTreeSet::new();
    for (index, meta) in physical.fixed_accounts.iter().enumerate() {
        if meta.is_signer {
            return Err(refusal(
                "route-manifest/fixed-signer",
                format!("fixed account {index} projects signer privilege"),
            ));
        }
        let expected_writable = index == 1;
        if meta.is_writable != expected_writable {
            return Err(refusal(
                "route-manifest/fixed-writable",
                format!("fixed account {index} projects noncanonical writable privilege"),
            ));
        }
        if meta.account.key == payer {
            return Err(refusal(
                "route-manifest/payer-aliases-fixed",
                format!("route payer aliases fixed account {index}"),
            ));
        }
        if !fixed_addresses.insert(meta.account.key) {
            return Err(refusal(
                "route-manifest/fixed-duplicate",
                format!("fixed account {index} duplicates an earlier fixed address"),
            ));
        }
        fixed.push(ManifestFixedCoordinateV3 {
            role: DIRECT_HOT_FIXED_ROLE_LABELS_V3[index],
            address: meta.account.key.to_string(),
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        });
    }

    if physical.runtime_accounts.len()
        != DIRECT_RUNTIME_FIXED_PREFIX_ROWS_V3 + DIRECT_HOT_FIXED_ROLE_LABELS_V3.len()
    {
        return Err(refusal(
            "route-manifest/runtime-count",
            format!(
                "route projects {} physical runtime accounts; the current shape is exactly {}",
                physical.runtime_accounts.len(),
                DIRECT_RUNTIME_FIXED_PREFIX_ROWS_V3 + DIRECT_HOT_FIXED_ROLE_LABELS_V3.len()
            ),
        ));
    }
    let tail = &physical.runtime_accounts[DIRECT_RUNTIME_FIXED_PREFIX_ROWS_V3..];
    let mut tail_addresses = std::collections::BTreeSet::new();
    let mut runtime = Vec::with_capacity(tail.len());
    for (index, meta) in tail.iter().enumerate() {
        if meta.is_signer != (index == 1) {
            return Err(refusal(
                "route-manifest/runtime-signer",
                format!("runtime tail account {index} projects noncanonical signer privilege"),
            ));
        }
        if index == 1 && (meta.account.key != payer || !meta.is_writable) {
            return Err(refusal(
                "route-manifest/runtime-payer",
                "runtime tail account 1 is not the writable route payer",
            ));
        }
        if !tail_addresses.insert(meta.account.key) {
            return Err(refusal(
                "route-manifest/runtime-duplicate",
                format!("runtime tail account {index} duplicates an earlier tail address"),
            ));
        }
        runtime.push(ManifestCoordinateV3 {
            address: meta.account.key.to_string(),
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        });
    }

    let lookup_table = planning.provision.lookup_table;
    if lookup_table == payer
        || fixed
            .iter()
            .any(|entry| entry.address == lookup_table.to_string())
        || runtime
            .iter()
            .any(|entry| entry.address == lookup_table.to_string())
    {
        return Err(refusal(
            "route-manifest/lookup-aliases",
            "lookup table aliases the payer or one Hot instruction account",
        ));
    }
    if planning.provision.authority != payer {
        return Err(refusal(
            "route-manifest/lookup-authority",
            "lookup table provision authority is not the route payer",
        ));
    }

    Ok(DirectHotRouteManifestDocumentV3 {
        format: DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3,
        payer: payer.to_string(),
        fixed_accounts: fixed,
        strategy_accounts: Vec::new(),
        runtime_accounts: runtime,
        lookup_tables: vec![lookup_table.to_string()],
        lookup_table_creation_slot: planning.provision.creation_slot.to_string(),
        checked_infrastructure: BASE64.encode(blob),
        checked_infrastructure_sha256: sha256_hex(blob),
    })
}

pub(crate) fn hot_route_manifest_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-direct-hot-route-manifest-v3 \
     --rpc-url DEVNET_HTTPS_URL \
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
     --session ABSOLUTE_JSON \
     --checked-execution-release ABS --expected-checked-execution-release-sha256 HEX64 \
     --registry-checked ABS --expected-registry-checked-sha256 HEX64 \
     --rent-checked ABS --expected-rent-checked-sha256 HEX64 \
     --output ABSOLUTE_NEW_JSON"
}

pub(crate) fn run_hot_route_manifest(arguments: Vec<String>) -> Result<()> {
    let usage = hot_route_manifest_usage();
    let mut walk = FlagWalkV1::parse(arguments, usage)?;
    let mut rpc = devnet_rpc(&mut walk, usage)?;
    let session = PathBuf::from(walk.take("--session", usage)?);
    let (_, multiprogram_bytes) = pinned_named(
        &mut walk,
        "--checked-execution-release",
        "--expected-checked-execution-release-sha256",
        "checked execution release",
        usage,
    )?;
    let (_, registry_checked_bytes) = pinned_named(
        &mut walk,
        "--registry-checked",
        "--expected-registry-checked-sha256",
        "registry checked release",
        usage,
    )?;
    let (_, rent_checked_bytes) = pinned_named(
        &mut walk,
        "--rent-checked",
        "--expected-rent-checked-sha256",
        "rent checked release",
        usage,
    )?;
    let output = PathBuf::from(walk.take("--output", usage)?);
    walk.finish()?;

    let validated = direct_trade::load_and_validate_manifests(&session, ExpectedClusterV1::Devnet)?;
    let session_checked = BASE64
        .decode(&validated.public.checked_execution_release_set_base64)
        .map_err(|error| Error::new(format!("session checked release base64: {error}")))?;
    if session_checked != multiprogram_bytes {
        return Err(refusal(
            "route-manifest/checked-release-differs",
            "supplied checked execution release differs from the session's embedded evidence",
        ));
    }

    let journal_root = direct_trade::journal_root_v1(&validated)?.ok_or_else(|| {
        refusal(
            "route-manifest/no-frozen-lookup",
            "the session has no lookup-create journal yet; run devnet-direct-trade-v1 --execute \
             through lookup-freeze before emitting the public route",
        )
    })?;
    let planning =
        direct_trade::collect_direct_trade_planning_v1(&mut rpc, &validated, Some(&journal_root))?;
    direct_trade::authenticate_frozen_lookup_v1(&planning)?;
    if planning.lookup_table.is_none() {
        return Err(refusal(
            "route-manifest/lookup-unobserved",
            "the frozen lookup table was not observed at the finalized snapshot",
        ));
    }

    let (blob, _decoded) = assemble_checked_infrastructure_v1(
        &mut rpc,
        &validated.plan,
        &multiprogram_bytes,
        &registry_checked_bytes,
        &rent_checked_bytes,
    )?;
    let document = project_manifest_document_v3(&planning, &blob)?;
    let mut text = serde_json::to_string_pretty(&document)
        .map_err(|error| Error::new(format!("route manifest serialization: {error}")))?;
    text.push('\n');
    if text.len() > 65_536 {
        return Err(refusal(
            "route-manifest/envelope-bytes",
            format!(
                "serialized manifest is {} bytes; the reader admits at most 65536",
                text.len()
            ),
        ));
    }
    write_create_new(&output, text.as_bytes())?;

    stdout_json(&json!({
        "schema": "dclutch-devnet-direct-hot-route-manifest-report-v1",
        "format": DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3,
        "output": output.display().to_string(),
        "bytes": text.len(),
        "sha256": sha256_hex(text.as_bytes()),
        "market": document.fixed_accounts[0].address,
        "payer": document.payer,
        "lookupTable": document.lookup_tables[0],
        "lookupTableCreationSlot": document.lookup_table_creation_slot,
        "checkedInfrastructureSha256": document.checked_infrastructure_sha256,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_market::capability_program::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn fixed_role_labels_cover_the_exact_generated_frame() {
        assert_eq!(
            DIRECT_HOT_FIXED_ROLE_LABELS_V3.len(),
            HOT_FIXED_ACCOUNT_COUNT_V3
        );
        assert_eq!(DIRECT_HOT_FIXED_ROLE_LABELS_V3[0], "Market");
        assert_eq!(DIRECT_HOT_FIXED_ROLE_LABELS_V3[1], "Direct root");
        assert_eq!(DIRECT_HOT_FIXED_ROLE_LABELS_V3[38], "Capability seal");
        let mut unique = std::collections::BTreeSet::new();
        for label in DIRECT_HOT_FIXED_ROLE_LABELS_V3 {
            assert!(unique.insert(label), "duplicate fixed-role label {label}");
        }
    }

    #[test]
    fn the_envelope_serializes_with_the_reader_key_vocabulary() {
        let document = DirectHotRouteManifestDocumentV3 {
            format: DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3,
            payer: Pubkey::new_unique().to_string(),
            fixed_accounts: vec![ManifestFixedCoordinateV3 {
                role: "Market",
                address: Pubkey::new_unique().to_string(),
                is_signer: false,
                is_writable: false,
            }],
            strategy_accounts: Vec::new(),
            runtime_accounts: vec![ManifestCoordinateV3 {
                address: Pubkey::new_unique().to_string(),
                is_signer: true,
                is_writable: true,
            }],
            lookup_tables: vec![Pubkey::new_unique().to_string()],
            lookup_table_creation_slot: 490_118_330_u64.to_string(),
            checked_infrastructure: "AAAA".to_string(),
            checked_infrastructure_sha256: "ab".repeat(32),
        };
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();
        let object = value.as_object().unwrap();
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "checkedInfrastructure",
                "checkedInfrastructureSha256",
                "fixedAccounts",
                "format",
                "lookupTableCreationSlot",
                "lookupTables",
                "payer",
                "runtimeAccounts",
                "strategyAccounts",
            ]
        );
        let fixed = object["fixedAccounts"][0].as_object().unwrap();
        let mut fixed_keys: Vec<&str> = fixed.keys().map(String::as_str).collect();
        fixed_keys.sort_unstable();
        assert_eq!(
            fixed_keys,
            vec!["address", "isSigner", "isWritable", "role"]
        );
        assert_eq!(object["lookupTableCreationSlot"], "490118330");
    }

    #[test]
    fn refusals_carry_their_pinned_bracketed_code() {
        let error = refusal("route-manifest/no-frozen-lookup", "not yet");
        let text = format!("{error}");
        assert!(
            text.contains("REFUSED: [route-manifest/no-frozen-lookup]"),
            "{text}"
        );
    }
}
