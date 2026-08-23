//! Offline composition of the one chain configuration accepted by `chain-serve`.
//!
//! The capability-profile checker remains the semantic owner. The local public
//! manifest remains the deployment-coordinate owner. This module only joins
//! their independently checked identities and emits a deterministic projection
//! configuration; it has no RPC, wallet, signing, persistence, or submission
//! capability.

use crate::{chain_server, repo_path, Result};
use clutch_local_real_pyth::account_index::CANONICAL_ACCOUNT_DECODER_SET;
use clutch_local_real_pyth::rpc_index::CanonicalFamily;
use serde_json::{json, Value};
use solana_address::Address;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const LOCAL_MANIFEST_SCHEMA: &str = "dragons-clutch/local-validator-public-manifest/v6";
const MAX_LOCAL_MANIFEST_BYTES: usize = 262_144;
const MAX_CAPABILITY_MANIFEST_BYTES: usize = 1_048_576;
const WORKFLOW_DOMAIN: &[u8] = b"dragons-clutch/operatord-chain-config-workflow/v2\0";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeploymentSlotPolicy {
    SynthesizedLocalZero,
    ObservedPublicPositive,
}

impl DeploymentSlotPolicy {
    const fn accepts(self, slot: u64) -> bool {
        match self {
            Self::SynthesizedLocalZero => slot == 0,
            Self::ObservedPublicPositive => slot != 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeOptions {
    pub local_release_manifest: PathBuf,
    pub capability_manifest: PathBuf,
    pub cluster_name: String,
    pub expected_genesis: String,
    pub rpc_http_url: String,
    pub rpc_websocket_url: String,
}

fn refuse_key_like_path(path: &Path, name: &str) -> Result<()> {
    if !path.is_absolute() || path == Path::new("/") {
        return Err(format!("{name} path must be narrow and absolute").into());
    }
    for component in path.components() {
        let value = match component {
            Component::RootDir => continue,
            Component::Normal(value) => value,
            _ => return Err(format!("{name} path contains a non-normal component").into()),
        };
        let value = value.to_string_lossy().to_ascii_lowercase();
        if value == ".solana"
            || value == "ephemeral-keys"
            || value == "id.json"
            || [
                "wallet",
                "keypair",
                "private-key",
                "private_key",
                "mnemonic",
                "seed",
                "secret",
                "keystore",
                "recovery-phrase",
                "recovery_phrase",
            ]
            .iter()
            .any(|marker| value.contains(*marker))
        {
            return Err(format!("{name} path is key-like and refused").into());
        }
    }
    Ok(())
}

fn resolve_existing_input(path: &Path, name: &str) -> Result<PathBuf> {
    refuse_key_like_path(path, name)?;
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(format!("{name} path has a symlink file leaf").into());
    }
    let resolved = std::fs::canonicalize(path)?;
    refuse_key_like_path(&resolved, name)?;
    if std::fs::symlink_metadata(&resolved)?.file_type().is_symlink() {
        return Err(format!("{name} resolved to a symlink file leaf").into());
    }
    Ok(resolved)
}

fn bounded_read_resolved(path: &Path, maximum: usize, name: &str) -> Result<Vec<u8>> {
    refuse_key_like_path(path, name)?;
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(format!("{name} resolved path became a symlink file leaf").into());
    }
    let bytes = std::fs::read(path)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(format!("{name} must contain 1..={maximum} bytes").into());
    }
    Ok(bytes)
}

fn bounded_read(path: &Path, maximum: usize, name: &str) -> Result<Vec<u8>> {
    let resolved = resolve_existing_input(path, name)?;
    bounded_read_resolved(&resolved, maximum, name)
}

fn parse_local_manifest(path: &Path) -> Result<BTreeMap<String, String>> {
    let resolved = resolve_existing_input(path, "local release manifest")?;
    let bytes = bounded_read_resolved(
        &resolved,
        MAX_LOCAL_MANIFEST_BYTES,
        "local release manifest",
    )?;
    let text = std::str::from_utf8(&bytes)?;
    let mut fields = BTreeMap::new();
    for (line_index, line) in text.lines().enumerate() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("local release manifest line {} has no '='", line_index + 1))?;
        if key.is_empty()
            || value.is_empty()
            || key
                .bytes()
                .any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'))
            || fields.insert(key.to_string(), value.to_string()).is_some()
        {
            return Err(format!(
                "local release manifest line {} is not canonical",
                line_index + 1
            )
            .into());
        }
    }
    if field(&fields, "schema")? != LOCAL_MANIFEST_SCHEMA
        || field(&fields, "network")? != "local-validator"
        || field(&fields, "release_coordinates")? != "sealed"
        || field(&fields, "decoder_set")? != CANONICAL_ACCOUNT_DECODER_SET
        || field(&fields, "signing")? != "not-exposed"
        || field(&fields, "submission")? != "not-exposed"
    {
        return Err("local release manifest is unsealed, unsupported, or authority-bearing".into());
    }
    for name in [
        "capability_manifest_sha256",
        "capability_profile_identity",
        "source_commit",
        "compiler_release_sha256",
        "source_neutral_sink",
        "clutch_program",
        "clutch_program_data",
        "clutch_deployment_slot",
        "clutch_elf_sha256",
        "clutch_elf_path",
    ] {
        field(&fields, name)?;
    }
    let marker = resolved
        .parent()
        .ok_or("local release manifest has no session directory")?
        .join("SESSION_OWNER");
    if bounded_read(&marker, 128, "local session owner marker")?.as_slice()
        != clutch_local_real_pyth::session::SESSION_MARKER.as_bytes()
    {
        return Err("local release manifest session owner marker mismatches".into());
    }
    Ok(fields)
}

fn field<'a>(fields: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str> {
    fields
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("local release manifest is missing {name}").into())
}

fn hash32(text: &str, name: &str) -> Result<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{name} must be exactly 32 lowercase hexadecimal bytes").into());
    }
    let mut output = [0_u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let decode = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        output[index] =
            (decode(pair[0]).ok_or("invalid hex")? << 4) | decode(pair[1]).ok_or("invalid hex")?;
    }
    if output == [0; 32] {
        return Err(format!("{name} must be nonzero").into());
    }
    Ok(output)
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct ExactManifestCopy {
    path: PathBuf,
}

impl ExactManifestCopy {
    fn create(bytes: &[u8]) -> Result<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for _ in 0..32 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "dragons-clutch-capability-manifest-{}-{nonce}-{sequence}.json",
                std::process::id()
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(mut file) => {
                    let mut exact_copy = Self { path };
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    file.set_permissions(std::fs::Permissions::from_mode(0o400))?;
                    exact_copy.path = resolve_existing_input(
                        &exact_copy.path,
                        "capability manifest checker handoff",
                    )?;
                    return Ok(exact_copy);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not exclusively create exact capability-manifest handoff".into())
    }

    fn verify_unchanged(&self, expected: &[u8]) -> Result<()> {
        if bounded_read(
            &self.path,
            MAX_CAPABILITY_MANIFEST_BYTES,
            "capability manifest checker handoff",
        )? != expected
        {
            return Err("capability-manifest checker handoff changed during validation".into());
        }
        Ok(())
    }
}

impl Drop for ExactManifestCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(crate) struct CheckedCapabilityRelease {
    summary: Value,
    pub(crate) manifest_sha256: [u8; 32],
    pub(crate) profile_identity: [u8; 32],
    pub(crate) source_commit: String,
    pub(crate) elf_sha256: [u8; 32],
}

pub(crate) fn checked_capability_release(path: &Path) -> Result<CheckedCapabilityRelease> {
    let bytes = bounded_read(path, MAX_CAPABILITY_MANIFEST_BYTES, "capability manifest")?;
    let exact_copy = ExactManifestCopy::create(&bytes)?;
    let checker = repo_path("programs/clutch-sbf/scripts/check_capability_profile.py");
    let repo = repo_path("");
    let output = Command::new("python3")
        .arg(checker)
        .arg(&exact_copy.path)
        .arg("--repo")
        .arg(repo)
        .arg("--require-deployable")
        .output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(2_048)
            .collect::<String>();
        return Err(format!("capability-profile checker refused release: {detail}").into());
    }
    exact_copy.verify_unchanged(&bytes)?;
    if output.stdout.is_empty() || output.stdout.len() > MAX_CAPABILITY_MANIFEST_BYTES {
        return Err("capability-profile checker output exceeds its bound".into());
    }
    let summary: Value = serde_json::from_slice(&output.stdout)?;
    if summary.get("deployment_eligible") != Some(&Value::Bool(true))
        || summary.get("release_declaration") != Some(&Value::Bool(false))
        || summary
            .get("planned_capabilities")
            .and_then(Value::as_array)
            .is_none_or(|rows| !rows.is_empty())
    {
        return Err("capability profile is not a completely linked deployable input".into());
    }
    let manifest_sha256 = hash32(
        summary_string(&summary, &["manifest_canonical_sha256"])?,
        "checked profile manifest_canonical_sha256",
    )?;
    let profile_identity = hash32(
        summary_string(&summary, &["profile_identity_sha256"] )?,
        "checked profile identity",
    )?;
    let source_commit = summary_string(&summary, &["measurement", "source_git_commit"])?;
    if !matches!(source_commit.len(), 40 | 64)
        || !source_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || source_commit.bytes().all(|byte| byte == b'0')
    {
        return Err("checked profile source commit is not canonical".into());
    }
    let elf_sha256 = hash32(
        summary_string(&summary, &["measurement", "elf_sha256"] )?,
        "checked profile ELF digest",
    )?;
    Ok(CheckedCapabilityRelease {
        summary,
        manifest_sha256,
        profile_identity,
        source_commit: source_commit.to_string(),
        elf_sha256,
    })
}

fn summary_string<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str> {
    let mut cursor = value;
    for name in path {
        cursor = cursor
            .get(name)
            .ok_or_else(|| format!("checked profile summary is missing {}", path.join(".")))?;
    }
    cursor
        .as_str()
        .ok_or_else(|| format!("checked profile summary {} is not a string", path.join(".")).into())
}

fn selected_families(summary: &Value) -> Result<Vec<CanonicalFamily>> {
    let mut families = BTreeSet::from([
        CanonicalFamily::Collateral,
        CanonicalFamily::Fees,
        CanonicalFamily::General,
        CanonicalFamily::Liveness,
    ]);
    let capabilities = summary
        .get("capabilities")
        .and_then(Value::as_array)
        .ok_or("checked profile summary has no capability rows")?;
    for row in capabilities {
        if row.get("linkage").and_then(Value::as_str) != Some("linked") {
            continue;
        }
        match row
            .get("slot")
            .and_then(Value::as_str)
            .ok_or("capability slot is absent")?
        {
            "retirement" => {
                families.insert(CanonicalFamily::PositionV3);
                families.insert(CanonicalFamily::ReplayV3);
            }
            "source-plane" => {
                families.insert(CanonicalFamily::Source);
            }
            "series-products" => {
                families.insert(CanonicalFamily::Series);
            }
            "recovery" => {
                families.insert(CanonicalFamily::Failure);
            }
            "structured-claim" => {
                families.insert(CanonicalFamily::StructuredClaim);
            }
            "liquidity-dealer" => {
                families.insert(CanonicalFamily::Dealer);
            }
            _ => {}
        }
    }
    for intent in enabled_intents(summary)? {
        match intent[0] {
            clutch_solana_layout::registry::GENERAL_V2_FAMILY_TAG => {
                families.insert(CanonicalFamily::General);
            }
            clutch_solana_layout::registry::STRUCTURED_CLAIM_FAMILY_TAG => {
                families.insert(CanonicalFamily::StructuredClaim);
            }
            clutch_solana_layout::registry::DEALER_FAMILY_TAG => {
                families.insert(CanonicalFamily::Dealer);
            }
            clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_TAG => {
                families.insert(CanonicalFamily::Source);
                families.insert(CanonicalFamily::Series);
            }
            clutch_solana_layout::registry::RECOVERY_FAMILY_TAG => {
                families.insert(CanonicalFamily::Failure);
            }
            clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_FAMILY_TAG => {
                families.insert(CanonicalFamily::Fractional);
            }
            clutch_solana_layout::registry::DIRECT_MARKET_FAMILY_TAG => {
                families.insert(CanonicalFamily::Direct);
                families.insert(CanonicalFamily::PositionV3);
                families.insert(CanonicalFamily::ReplayV3);
            }
            _ => {
                return Err(
                    "checked profile enables a family unknown to the current decoder set".into(),
                )
            }
        }
    }
    Ok(families.into_iter().collect())
}

fn enabled_intents(summary: &Value) -> Result<Vec<[u8; 3]>> {
    let rows = summary
        .get("central_registry")
        .and_then(|value| value.get("enabled_intent_triples"))
        .and_then(Value::as_array)
        .ok_or("checked profile summary has no enabled intent triples")?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        let items = row.as_array().ok_or("enabled intent is not a triple")?;
        if items.len() != 3 {
            return Err("enabled intent is not a triple".into());
        }
        let mut triple = [0_u8; 3];
        for (index, item) in items.iter().enumerate() {
            triple[index] = u8::try_from(
                item.as_u64()
                    .ok_or("enabled intent component is not unsigned")?,
            )?;
        }
        if triple[0] == 0
            || triple[1] == 0
            || output.last().is_some_and(|previous| previous >= &triple)
        {
            return Err("enabled intent triples are not strictly canonical".into());
        }
        output.push(triple);
    }
    Ok(output)
}

fn sha_join(parts: &[&[u8]]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for part in parts {
        bytes.extend_from_slice(part);
    }
    solana_sha256_hasher::hash(&bytes).to_bytes()
}

pub(crate) fn validate_upgradeable_release_coordinates(
    program: Address,
    program_data: Address,
) -> Result<()> {
    let loader = Address::new_from_array(clutch_sbf::loader_state::UPGRADEABLE_LOADER_ID);
    let expected = Address::find_program_address(&[program.as_ref()], &loader).0;
    if program == Address::default() || program_data != expected {
        return Err("ProgramData is not the canonical upgradeable-loader derivation".into());
    }
    Ok(())
}

pub fn compose(options: &ComposeOptions) -> Result<String> {
    let local = parse_local_manifest(&options.local_release_manifest)?;
    if field(&local, "rpc_http")? != options.rpc_http_url
        || field(&local, "rpc_websocket")? != options.rpc_websocket_url
    {
        return Err("explicit RPC endpoints differ from the sealed local session manifest".into());
    }
    let checked = checked_capability_release(&options.capability_manifest)?;
    if hash32(
        field(&local, "capability_manifest_sha256")?,
        "capability_manifest_sha256",
    )? != checked.manifest_sha256
    {
        return Err("sealed local release pins a different canonical capability manifest".into());
    }
    if hash32(
        field(&local, "capability_profile_identity")?,
        "capability_profile_identity",
    )? != checked.profile_identity
    {
        return Err("sealed local release pins a different capability-profile identity".into());
    }
    if hash32(field(&local, "clutch_elf_sha256")?, "clutch_elf_sha256")?
        != checked.elf_sha256
    {
        return Err("sealed local ELF digest differs from checked profile measurement".into());
    }
    if field(&local, "source_commit")? != checked.source_commit.as_str() {
        return Err("sealed local source commit differs from checked measurement source".into());
    }
    let elf_path = Path::new(field(&local, "clutch_elf_path")?);
    if !elf_path.is_absolute() || elf_path == Path::new("/") {
        return Err("sealed local ELF path is not an absolute file path".into());
    }
    let elf = bounded_read(elf_path, 10 * 1024 * 1024, "deployed ELF")?;
    if solana_sha256_hasher::hash(&elf).to_bytes() != checked.elf_sha256 {
        return Err("deployed ELF bytes differ from the sealed and measured digest".into());
    }
    let program = Address::from_str(field(&local, "clutch_program")?)?;
    let program_data = Address::from_str(field(&local, "clutch_program_data")?)?;
    validate_upgradeable_release_coordinates(program, program_data)?;
    if field(&local, "clutch_deployment_slot")? != "0" {
        return Err("sealed local deployment slot must be canonical zero".into());
    }
    let slot = 0;
    compose_checked_chain_config(
        &checked,
        &options.cluster_name,
        &options.expected_genesis,
        &options.rpc_http_url,
        &options.rpc_websocket_url,
        program,
        program_data,
        slot,
        DeploymentSlotPolicy::SynthesizedLocalZero,
        field(&local, "source_neutral_sink")?,
        field(&local, "compiler_release_sha256")?,
        &checked.manifest_sha256,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compose_checked_chain_config(
    checked: &CheckedCapabilityRelease,
    cluster_name: &str,
    expected_genesis: &str,
    rpc_http_url: &str,
    rpc_websocket_url: &str,
    program: Address,
    program_data: Address,
    slot: u64,
    slot_policy: DeploymentSlotPolicy,
    source_neutral_sink: &str,
    compiler_release_sha256: &str,
    workflow_binding: &[u8; 32],
) -> Result<String> {
    let intents = enabled_intents(&checked.summary)?;
    let families = selected_families(&checked.summary)?;
    let sink = Address::from_str(source_neutral_sink)?;
    if sink == Address::default() {
        return Err("source neutral sink is zero".into());
    }
    hash32(compiler_release_sha256, "compiler_release_sha256")?;
    validate_upgradeable_release_coordinates(program, program_data)?;
    if !slot_policy.accepts(slot)
        || workflow_binding == &[0; 32]
        || cluster_name.is_empty()
        || expected_genesis.is_empty()
        || rpc_http_url.is_empty()
        || rpc_websocket_url.is_empty()
    {
        return Err("chain configuration coordinates are incomplete".into());
    }
    let workflow_id = sha_join(&[
        WORKFLOW_DOMAIN,
        workflow_binding,
        expected_genesis.as_bytes(),
        program.as_ref(),
    ]);
    let value = json!({
        "schema": "dragons-clutch/operatord-chain-config/v2",
        "decoderSet": CANONICAL_ACCOUNT_DECODER_SET,
        "cluster": {
            "name": cluster_name,
            "genesisHash": expected_genesis,
            "rpcHttpUrl": rpc_http_url,
            "rpcWebsocketUrl": rpc_websocket_url
        },
        "releases": [{
            "programId": program.to_string(),
            "programData": program_data.to_string(),
            "elfSha256": hex(checked.elf_sha256),
            "deploymentSlot": slot.to_string(),
            "releaseManifestSha256": hex(checked.manifest_sha256),
            "capabilityProfileId": hex(checked.profile_identity),
            "sourceCommit": checked.source_commit.as_str(),
            "enabledIntents": intents.iter().map(|triple| json!({
                "familyTag": triple[0].to_string(),
                "familyVersion": triple[1].to_string(),
                "localAction": triple[2].to_string()
            })).collect::<Vec<_>>(),
            "families": families.iter().map(|family| family.name()).collect::<Vec<_>>()
        }],
        "sourceNeutralSink": source_neutral_sink,
        "workflowId": hex(workflow_id),
        "maximumKeeperActions": "4096",
        "bounds": {
            "maximumAccountsPerScan": "65536",
            "maximumAccountDataBytes": "1048576",
            "maximumTotalResponseBytes": "268435456",
            "maximumSubscriptions": "256",
            "maximumAddresses": "262144",
            "maximumVersionsPerAddress": "64",
            "maximumForkNodes": "262144"
        },
        "pollingIntervalMilliseconds": "5000",
        "rpcTimeoutSeconds": "30",
        "websocketReconnectInitialMilliseconds": "500",
        "websocketReconnectMaximumMilliseconds": "30000",
        "compilerReleaseSha256": compiler_release_sha256
    });
    let output = serde_json::to_string_pretty(&value)? + "\n";
    chain_server::validate_chain_config_bytes(output.as_bytes())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_hash_and_intent_selection_do_not_depend_on_browser_input() {
        let summary = json!({
            "central_registry": {"enabled_intent_triples": [[74, 1, 26], [76, 1, 1], [78, 1, 1], [79, 1, 1], [80, 1, 1]]},
            "capabilities": [
                {"slot": "source-plane", "linkage": "linked"},
                {"slot": "liquidity-dealer", "linkage": "linked"},
                {"slot": "recovery", "linkage": "linked"}
            ]
        });
        assert_eq!(enabled_intents(&summary).unwrap()[0], [74, 1, 26]);
        let families = selected_families(&summary).unwrap();
        assert!(families.contains(&CanonicalFamily::Source));
        assert!(families.contains(&CanonicalFamily::Dealer));
        assert!(families.contains(&CanonicalFamily::Failure));
        assert!(families.contains(&CanonicalFamily::Fractional));
        assert!(families.contains(&CanonicalFamily::Direct));
        assert!(families.contains(&CanonicalFamily::PositionV3));
        assert!(families.contains(&CanonicalFamily::ReplayV3));
    }

    #[test]
    fn deployment_slot_policies_are_disjoint() {
        assert!(DeploymentSlotPolicy::SynthesizedLocalZero.accepts(0));
        assert!(!DeploymentSlotPolicy::SynthesizedLocalZero.accepts(1));
        assert!(!DeploymentSlotPolicy::ObservedPublicPositive.accepts(0));
        assert!(DeploymentSlotPolicy::ObservedPublicPositive.accepts(1));
    }
}
