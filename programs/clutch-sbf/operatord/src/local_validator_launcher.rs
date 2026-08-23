//! Exact local-validator launch composition for the chain-attached daemon.
//!
//! This is deliberately separate from the historical mock modes.  It accepts
//! only explicit built-release, capability-profile, compiler, neutral-sink,
//! source, validator, and genesis coordinates; creates the v6 sealed session
//! through its semantic owner; composes the read-only chain configuration;
//! and can supervise that exact validator while `chain-serve` runs.  It never
//! reads a Solana CLI wallet, creates a key, signs, submits, deploys, or calls
//! an RPC endpoint during preparation.

use crate::compose_chain_config::{
    self, checked_capability_release, validate_upgradeable_release_coordinates,
};
use crate::{chain_server, Result};
use clutch_local_real_pyth::session::{
    CheckedChainReleaseBinding, LocalGenesisAccountFile, LocalProgramRelease, LocalSessionConfig,
    LocalValidatorInvocation, LocalValidatorPorts, RealSourceAcquisitionV3, RealSourceConfigV3,
    SessionLayout,
};
use serde::Deserialize;
use serde_json::json;
use solana_address::Address;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};

pub const LOCAL_LAUNCH_CONFIG_SCHEMA: &str =
    "dragons-clutch/local-validator-launch-config/v1";
const LOCAL_LAUNCH_PLAN_SCHEMA: &str = "dragons-clutch/local-validator-launch-plan/v1";
const MAX_CONFIG_BYTES: usize = 262_144;
const MAX_CAPABILITY_MANIFEST_BYTES: usize = 1_048_576;
const MAX_ELF_BYTES: usize = 10 * 1024 * 1024;
const MAX_VALIDATOR_BYTES: usize = 256 * 1024 * 1024;
const MAX_GENESIS_ACCOUNT_BYTES: usize = 16 * 1024 * 1024;
const MAX_EXTERNAL_PROGRAMS: usize = 16;
const MAX_EXTERNAL_PROGRAM_BYTES: usize = 80 * 1024 * 1024;
const MAX_GENESIS_ACCOUNTS: usize = 256;
const MAX_GENESIS_ACCOUNTS_BYTES: usize = 64 * 1024 * 1024;
const MAX_STAGED_INPUT_BYTES: usize = 384 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalLaunchOptions {
    pub config: PathBuf,
    pub capability_manifest: PathBuf,
    pub server_port: u16,
    pub statics: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalLaunchWire {
    schema: String,
    session_root: PathBuf,
    validator: ValidatorWire,
    cluster: ClusterWire,
    release: ReleaseWire,
    external_programs: Vec<ExternalProgramWire>,
    source: SourceWire,
    mint_authority: String,
    warp_slot: String,
    genesis_accounts: Vec<GenesisAccountWire>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatorWire {
    binary: PathBuf,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClusterWire {
    name: String,
    expected_genesis_hash: Option<String>,
    rpc_port: String,
    rpc_websocket_port: String,
    faucet_port: String,
    gossip_port: String,
    dynamic_port_start: String,
    dynamic_port_end: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseWire {
    program_id: String,
    program_data: String,
    deployment_slot: String,
    elf_path: PathBuf,
    compiler_release_sha256: String,
    source_neutral_sink: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalProgramWire {
    program_id: String,
    program_data: String,
    deployment_slot: String,
    elf_sha256: String,
    elf_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceWire {
    receiver_program: String,
    receiver_program_data: String,
    receiver_deployment_slot: String,
    receiver_config: String,
    receiver_release_sha256: String,
    parser_program: String,
    parser_program_data: String,
    parser_deployment_slot: String,
    parser_config: String,
    parser_release_sha256: String,
    feed_account: String,
    feed_id: String,
    transport_program: String,
    transport_program_data: String,
    transport_deployment_slot: String,
    transport_release_sha256: String,
    source_spec_id: String,
    acquisition: AcquisitionWire,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcquisitionWire {
    mode: String,
    capture_manifest_sha256: Option<String>,
    https_rpc_url: Option<String>,
    maximum_account_reads: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenesisAccountWire {
    role: String,
    address: String,
    account_json: PathBuf,
    body_sha256: String,
}

struct PreparedLocalChain {
    layout: SessionLayout,
    invocation: LocalValidatorInvocation,
    chain_config: PathBuf,
    capability_manifest: PathBuf,
    expected_genesis_hash: Option<String>,
    rpc_http_url: String,
    rpc_websocket_url: String,
    rpc_port: u16,
    faucet_port: u16,
    provenance_validator: PathBuf,
    validator_sha256: [u8; 32],
    staged_executables: Vec<StagedExecutable>,
}

struct ValidatorGuard(Child);

struct StagedInput {
    source: PathBuf,
    destination: PathBuf,
    expected_sha256: [u8; 32],
    maximum_bytes: usize,
    mode: u32,
    name: &'static str,
}

struct StagedExecutable {
    path: PathBuf,
    expected_sha256: [u8; 32],
    maximum_bytes: usize,
    name: &'static str,
}

impl Drop for ValidatorGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
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

fn digest(text: &str, name: &str) -> Result<[u8; 32]> {
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

fn positive_u64(text: &str, name: &str) -> Result<u64> {
    if text.is_empty()
        || text.starts_with('0')
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{name} must be a canonical positive decimal integer").into());
    }
    let value: u64 = text.parse()?;
    if value == 0 {
        return Err(format!("{name} must be positive").into());
    }
    Ok(value)
}

fn local_synthesized_slot(text: &str, name: &str) -> Result<u64> {
    if text != "0" {
        return Err(format!("{name} must be canonical zero for --bpf-program genesis").into());
    }
    Ok(0)
}

fn port(text: &str, name: &str) -> Result<u16> {
    Ok(u16::try_from(positive_u64(text, name)?)?)
}

fn address(text: &str, name: &str) -> Result<Address> {
    let value = Address::from_str(text)?;
    if value == Address::default() {
        return Err(format!("{name} must be nonzero").into());
    }
    Ok(value)
}

fn verify_file_digest(
    path: &Path,
    expected: [u8; 32],
    maximum: usize,
    name: &str,
) -> Result<usize> {
    let resolved = resolve_existing_input(path, name)?;
    if resolved.as_path() != path {
        return Err(format!("{name} resolved path changed").into());
    }
    let bytes = bounded_read_resolved(&resolved, maximum, name)?;
    if solana_sha256_hasher::hash(&bytes).to_bytes() != expected {
        return Err(format!("{name} bytes disagree with their exact digest").into());
    }
    Ok(bytes.len())
}

fn add_bounded_size(total: &mut usize, size: usize, maximum: usize, name: &str) -> Result<()> {
    *total = total
        .checked_add(size)
        .ok_or_else(|| format!("{name} byte count overflowed"))?;
    if *total > maximum {
        return Err(format!("{name} bytes exceed the aggregate {maximum}-byte bound").into());
    }
    Ok(())
}

fn require_bounded_count(count: usize, maximum: usize, name: &str) -> Result<()> {
    if count > maximum {
        return Err(format!("{name} exceeds the {maximum}-entry bound").into());
    }
    Ok(())
}

fn validate_genesis_account_json(path: &Path, allowed_owners: &BTreeSet<Address>) -> Result<()> {
    let bytes = bounded_read_resolved(path, MAX_GENESIS_ACCOUNT_BYTES, "genesis account JSON")?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let owner = value
        .get("owner")
        .and_then(serde_json::Value::as_str)
        .ok_or("genesis account JSON is missing its owner")?;
    let owner = address(owner, "genesis account owner")?;
    if !allowed_owners.contains(&owner)
        || value.get("executable").and_then(serde_json::Value::as_bool) != Some(false)
    {
        return Err(
            "genesis account may contain only non-executable state owned by a loaded external release"
                .into(),
        );
    }
    Ok(())
}

fn acquisition(wire: AcquisitionWire) -> Result<RealSourceAcquisitionV3> {
    match wire.mode.as_str() {
        "pinned-local-capture"
            if wire.https_rpc_url.is_none() && wire.maximum_account_reads.is_none() =>
        {
            Ok(RealSourceAcquisitionV3::PinnedLocalCapture {
                capture_manifest_sha256: digest(
                    wire.capture_manifest_sha256
                        .as_deref()
                        .ok_or("pinned capture acquisition is missing its digest")?,
                    "capture_manifest_sha256",
                )?,
            })
        }
        "bounded-public-read" if wire.capture_manifest_sha256.is_none() => {
            Ok(RealSourceAcquisitionV3::BoundedPublicRead {
                https_rpc_url: wire
                    .https_rpc_url
                    .ok_or("bounded public acquisition is missing its URL")?,
                maximum_account_reads: u16::try_from(positive_u64(
                    wire.maximum_account_reads
                        .as_deref()
                        .ok_or("bounded public acquisition is missing its read bound")?,
                    "maximum_account_reads",
                )?)?,
            })
        }
        _ => Err("source acquisition mode and fields are not canonical".into()),
    }
}

fn release(
    wire: ExternalProgramWire,
    index: usize,
    sealed_inputs: &Path,
) -> Result<(LocalProgramRelease, StagedInput, usize)> {
    let elf_source = resolve_existing_input(&wire.elf_path, "external program ELF")?;
    if elf_source.extension().and_then(|value| value.to_str()) != Some("so") {
        return Err("external program ELF input must be an absolute .so path".into());
    }
    let elf_sha256 = digest(&wire.elf_sha256, "external elf_sha256")?;
    let elf_bytes = verify_file_digest(
        &elf_source,
        elf_sha256,
        MAX_ELF_BYTES,
        "external program ELF",
    )?;
    let destination = sealed_inputs.join(format!("external-program-{index}.so"));
    let program_id = address(&wire.program_id, "external program_id")?;
    let program_data = address(&wire.program_data, "external program_data")?;
    validate_upgradeable_release_coordinates(program_id, program_data)?;
    let release = LocalProgramRelease {
        program_id,
        program_data,
        deployment_slot: local_synthesized_slot(
            &wire.deployment_slot,
            "external deployment_slot",
        )?,
        elf_sha256,
        elf_path: destination.clone(),
    };
    Ok((
        release,
        StagedInput {
            source: elf_source,
            destination,
            expected_sha256: elf_sha256,
            maximum_bytes: MAX_ELF_BYTES,
            mode: 0o400,
            name: "external program ELF",
        },
        elf_bytes,
    ))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn open_new_log(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?)
}

fn utf8_argument(value: &OsStr) -> Result<String> {
    Ok(value
        .to_str()
        .ok_or("validator argument is not UTF-8")?
        .to_string())
}

fn stage_input(input: StagedInput) -> Result<()> {
    let resolved = resolve_existing_input(&input.source, input.name)?;
    if resolved.as_path() != input.source.as_path() {
        return Err(format!("{} resolved path changed before sealed staging", input.name).into());
    }
    let bytes = bounded_read_resolved(&resolved, input.maximum_bytes, input.name)?;
    if solana_sha256_hasher::hash(&bytes).to_bytes() != input.expected_sha256 {
        return Err(format!("{} changed before sealed staging", input.name).into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(input.mode)
        .open(&input.destination)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn prepare(options: &LocalLaunchOptions) -> Result<PreparedLocalChain> {
    if options.config.extension().and_then(|value| value.to_str()) != Some("json")
        || options
            .capability_manifest
            .extension()
            .and_then(|value| value.to_str())
            != Some("json")
    {
        return Err("local launch and capability inputs must be explicit .json files".into());
    }
    let config_path = resolve_existing_input(&options.config, "local launch config")?;
    let config_bytes = bounded_read_resolved(
        &config_path,
        MAX_CONFIG_BYTES,
        "local launch config",
    )?;
    let wire: LocalLaunchWire = serde_json::from_slice(&config_bytes)?;
    if wire.schema != LOCAL_LAUNCH_CONFIG_SCHEMA || wire.cluster.name != "local-validator" {
        return Err("local launch config has an unsupported schema or network".into());
    }
    require_bounded_count(
        wire.external_programs.len(),
        MAX_EXTERNAL_PROGRAMS,
        "external_programs",
    )?;
    require_bounded_count(
        wire.genesis_accounts.len(),
        MAX_GENESIS_ACCOUNTS,
        "genesis_accounts",
    )?;
    if let Some(expected) = wire.cluster.expected_genesis_hash.as_deref() {
        address(expected, "expected_genesis_hash")?;
    }
    let capability_manifest =
        resolve_existing_input(&options.capability_manifest, "capability manifest")?;
    let capability_bytes = bounded_read_resolved(
        &capability_manifest,
        MAX_CAPABILITY_MANIFEST_BYTES,
        "capability manifest",
    )?;
    let checked = checked_capability_release(&capability_manifest)?;
    let mut staged_input_bytes = 0;
    add_bounded_size(
        &mut staged_input_bytes,
        capability_bytes.len(),
        MAX_STAGED_INPUT_BYTES,
        "staged input",
    )?;
    let session_root = wire.session_root.clone();
    let sealed_inputs = session_root.join("sealed-inputs");
    let clutch_elf_source =
        resolve_existing_input(&wire.release.elf_path, "Clutch release ELF")?;
    if clutch_elf_source.extension().and_then(|value| value.to_str()) != Some("so") {
        return Err("Clutch release ELF input must be an absolute .so path".into());
    }
    let clutch_elf_path = sealed_inputs.join("clutch_sbf.so");
    let clutch_elf_bytes = verify_file_digest(
        &clutch_elf_source,
        checked.elf_sha256,
        MAX_ELF_BYTES,
        "Clutch release ELF",
    )?;
    add_bounded_size(
        &mut staged_input_bytes,
        clutch_elf_bytes,
        MAX_STAGED_INPUT_BYTES,
        "staged input",
    )?;
    let validator_source =
        resolve_existing_input(&wire.validator.binary, "local validator binary")?;
    let validator_binary = sealed_inputs.join("solana-test-validator");
    let validator_sha256 = digest(&wire.validator.sha256, "validator sha256")?;
    let validator_bytes = verify_file_digest(
        &validator_source,
        validator_sha256,
        MAX_VALIDATOR_BYTES,
        "local validator binary",
    )?;
    add_bounded_size(
        &mut staged_input_bytes,
        validator_bytes,
        MAX_STAGED_INPUT_BYTES,
        "staged input",
    )?;

    let ports = LocalValidatorPorts {
        rpc: port(&wire.cluster.rpc_port, "rpc_port")?,
        rpc_websocket: port(&wire.cluster.rpc_websocket_port, "rpc_websocket_port")?,
        faucet: port(&wire.cluster.faucet_port, "faucet_port")?,
        gossip: port(&wire.cluster.gossip_port, "gossip_port")?,
        dynamic_start: port(&wire.cluster.dynamic_port_start, "dynamic_port_start")?,
        dynamic_end: port(&wire.cluster.dynamic_port_end, "dynamic_port_end")?,
    };
    ports.validate()?;
    let clutch_program = address(&wire.release.program_id, "program_id")?;
    let clutch_program_data = address(&wire.release.program_data, "program_data")?;
    validate_upgradeable_release_coordinates(clutch_program, clutch_program_data)?;
    let clutch_release = LocalProgramRelease {
        program_id: clutch_program,
        program_data: clutch_program_data,
        deployment_slot: local_synthesized_slot(
            &wire.release.deployment_slot,
            "deployment_slot",
        )?,
        elf_sha256: checked.elf_sha256,
        elf_path: clutch_elf_path.clone(),
    };
    let mut staged_inputs = vec![
        StagedInput {
            source: clutch_elf_source,
            destination: clutch_elf_path.clone(),
            expected_sha256: checked.elf_sha256,
            maximum_bytes: MAX_ELF_BYTES,
            mode: 0o400,
            name: "Clutch release ELF",
        },
        StagedInput {
            source: validator_source.clone(),
            destination: validator_binary.clone(),
            expected_sha256: validator_sha256,
            maximum_bytes: MAX_VALIDATOR_BYTES,
            mode: 0o500,
            name: "local validator binary",
        },
    ];
    let mut staged_executables = vec![
        StagedExecutable {
            path: clutch_elf_path,
            expected_sha256: checked.elf_sha256,
            maximum_bytes: MAX_ELF_BYTES,
            name: "sealed Clutch release ELF",
        },
        StagedExecutable {
            path: validator_binary.clone(),
            expected_sha256: validator_sha256,
            maximum_bytes: MAX_VALIDATOR_BYTES,
            name: "sealed local validator binary",
        },
    ];
    let mut external_program_bytes = 0;
    let mut external_program_releases = Vec::with_capacity(wire.external_programs.len());
    for (index, external) in wire.external_programs.into_iter().enumerate() {
        let (program, staged, byte_count) = release(external, index, &sealed_inputs)?;
        add_bounded_size(
            &mut external_program_bytes,
            byte_count,
            MAX_EXTERNAL_PROGRAM_BYTES,
            "external program",
        )?;
        add_bounded_size(
            &mut staged_input_bytes,
            byte_count,
            MAX_STAGED_INPUT_BYTES,
            "staged input",
        )?;
        staged_executables.push(StagedExecutable {
            path: program.elf_path.clone(),
            expected_sha256: program.elf_sha256,
            maximum_bytes: MAX_ELF_BYTES,
            name: "sealed external program ELF",
        });
        external_program_releases.push(program);
        staged_inputs.push(staged);
    }
    let source = RealSourceConfigV3 {
        receiver_program: address(&wire.source.receiver_program, "receiver_program")?,
        receiver_program_data: address(
            &wire.source.receiver_program_data,
            "receiver_program_data",
        )?,
        receiver_deployment_slot: local_synthesized_slot(
            &wire.source.receiver_deployment_slot,
            "receiver_deployment_slot",
        )?,
        receiver_config: address(&wire.source.receiver_config, "receiver_config")?,
        receiver_release_sha256: digest(
            &wire.source.receiver_release_sha256,
            "receiver_release_sha256",
        )?,
        parser_program: address(&wire.source.parser_program, "parser_program")?,
        parser_program_data: address(&wire.source.parser_program_data, "parser_program_data")?,
        parser_deployment_slot: local_synthesized_slot(
            &wire.source.parser_deployment_slot,
            "parser_deployment_slot",
        )?,
        parser_config: address(&wire.source.parser_config, "parser_config")?,
        parser_release_sha256: digest(
            &wire.source.parser_release_sha256,
            "parser_release_sha256",
        )?,
        feed_account: address(&wire.source.feed_account, "feed_account")?,
        feed_id: digest(&wire.source.feed_id, "feed_id")?,
        transport_program: address(&wire.source.transport_program, "transport_program")?,
        transport_program_data: address(
            &wire.source.transport_program_data,
            "transport_program_data",
        )?,
        transport_deployment_slot: local_synthesized_slot(
            &wire.source.transport_deployment_slot,
            "transport_deployment_slot",
        )?,
        transport_release_sha256: digest(
            &wire.source.transport_release_sha256,
            "transport_release_sha256",
        )?,
        source_spec_id: digest(&wire.source.source_spec_id, "source_spec_id")?,
        acquisition: acquisition(wire.source.acquisition)?,
    };
    let source_neutral_sink = address(&wire.release.source_neutral_sink, "source_neutral_sink")?;
    let compiler_release_sha256 = digest(
        &wire.release.compiler_release_sha256,
        "compiler_release_sha256",
    )?;
    let config = LocalSessionConfig {
        root: session_root,
        ports,
        clutch_release,
        checked_chain_release: CheckedChainReleaseBinding {
            capability_manifest_sha256: checked.manifest_sha256,
            capability_profile_id: checked.profile_identity,
            source_commit: checked.source_commit.clone(),
            compiler_release_sha256,
            source_neutral_sink,
        },
        external_program_releases,
        source,
    };
    config.validate()?;

    let allowed_genesis_owners = config
        .external_program_releases
        .iter()
        .map(|release| release.program_id)
        .collect::<BTreeSet<_>>();
    let mut genesis_account_bytes = 0;
    let mut genesis_accounts = Vec::with_capacity(wire.genesis_accounts.len());
    for (index, account) in wire.genesis_accounts.into_iter().enumerate() {
        let account_json =
            resolve_existing_input(&account.account_json, "genesis account JSON")?;
        if account_json
            .extension()
            .and_then(|value| value.to_str())
            != Some("json")
        {
            return Err("genesis account input must be an explicit .json file".into());
        }
        let body_sha256 = digest(&account.body_sha256, "genesis body_sha256")?;
        validate_genesis_account_json(&account_json, &allowed_genesis_owners)?;
        let byte_count = verify_file_digest(
            &account_json,
            body_sha256,
            MAX_GENESIS_ACCOUNT_BYTES,
            "genesis account JSON",
        )?;
        add_bounded_size(
            &mut genesis_account_bytes,
            byte_count,
            MAX_GENESIS_ACCOUNTS_BYTES,
            "genesis account",
        )?;
        add_bounded_size(
            &mut staged_input_bytes,
            byte_count,
            MAX_STAGED_INPUT_BYTES,
            "staged input",
        )?;
        let destination = sealed_inputs.join(format!("genesis-account-{index}.json"));
        staged_inputs.push(StagedInput {
            source: account_json,
            destination: destination.clone(),
            expected_sha256: body_sha256,
            maximum_bytes: MAX_GENESIS_ACCOUNT_BYTES,
            mode: 0o400,
            name: "genesis account JSON",
        });
        genesis_accounts.push(LocalGenesisAccountFile {
            role: account.role,
            address: address(&account.address, "genesis account address")?,
            account_json: destination,
            body_sha256,
        });
    }
    let mint_authority = address(&wire.mint_authority, "mint_authority")?;
    let warp_slot = positive_u64(&wire.warp_slot, "warp_slot")?;
    let layout = SessionLayout::initialize(&config)?;
    let prepared_parts: Result<(LocalValidatorInvocation, PathBuf)> = (|| {
        std::fs::create_dir(&sealed_inputs)?;
        std::fs::set_permissions(
            &sealed_inputs,
            std::fs::Permissions::from_mode(0o700),
        )?;
        for input in staged_inputs {
            stage_input(input)?;
        }
        let sealed_capability_manifest = sealed_inputs.join("capability-manifest.json");
        write_new(&sealed_capability_manifest, &capability_bytes)?;
        let staged_checked = checked_capability_release(&sealed_capability_manifest)?;
        if staged_checked.manifest_sha256 != checked.manifest_sha256
            || staged_checked.profile_identity != checked.profile_identity
            || staged_checked.source_commit != checked.source_commit
            || staged_checked.elf_sha256 != checked.elf_sha256
        {
            return Err("sealed capability-manifest copy changed release coordinates".into());
        }
        let invocation = LocalValidatorInvocation::new(
            validator_binary,
            &config,
            &layout,
            mint_authority,
            warp_slot,
            &genesis_accounts,
        )?;
        let chain_config = layout.root().join("operatord-chain.json");
        let arguments = invocation
            .arguments()
            .iter()
            .map(|value| utf8_argument(value.as_os_str()))
            .collect::<Result<Vec<_>>>()?;
        let launch_plan = serde_json::to_vec_pretty(&json!({
            "schema": LOCAL_LAUNCH_PLAN_SCHEMA,
            "network": "local-validator",
            "validatorSha256": wire.validator.sha256,
            "validatorExecutable": invocation.executable().display().to_string(),
            "validatorArguments": arguments,
            "publicSessionManifest": layout.public_manifest().display().to_string(),
            "plannedOperatordChainConfig": chain_config.display().to_string(),
            "wallet": "not-read-or-created",
            "signing": "not-exposed",
            "submission": "not-exposed",
            "deployment": "not-exposed"
        }))?;
        let launch_plan_path = layout.root().join("local-launch-plan.json");
        write_new(&launch_plan_path, &[launch_plan, vec![b'\n']].concat())?;
        Ok((invocation, chain_config))
    })();
    match prepared_parts {
        Ok((invocation, chain_config)) => Ok(PreparedLocalChain {
            layout,
            invocation,
            chain_config,
            capability_manifest: sealed_inputs.join("capability-manifest.json"),
            expected_genesis_hash: wire.cluster.expected_genesis_hash,
            rpc_http_url: ports.rpc_http(),
            rpc_websocket_url: ports.rpc_websocket(),
            rpc_port: ports.rpc,
            faucet_port: ports.faucet,
            provenance_validator: validator_source,
            validator_sha256,
            staged_executables,
        }),
        Err(error) => {
            layout.destroy()?;
            Err(error)
        }
    }
}

pub fn prepare_only(options: &LocalLaunchOptions) -> Result<String> {
    let prepared = prepare(options)?;
    Ok(serde_json::to_string_pretty(&json!({
        "schema": LOCAL_LAUNCH_PLAN_SCHEMA,
        "sessionRoot": prepared.layout.root().display().to_string(),
        "publicSessionManifest": prepared.layout.public_manifest().display().to_string(),
        "plannedOperatordChainConfig": prepared.chain_config.display().to_string(),
        "validatorInvocation": prepared.layout.root().join("local-launch-plan.json").display().to_string(),
        "status": "prepared-not-started-chain-config-awaits-observed-local-genesis",
        "wallet": "not-read-or-created",
        "signing": "not-exposed",
        "submission": "not-exposed",
        "deployment": "not-exposed"
    }))? + "\n")
}

fn verify_pinned_validator(prepared: &PreparedLocalChain) -> Result<()> {
    let provenance_validator = resolve_existing_input(
        &prepared.provenance_validator,
        "provenance validator binary",
    )?;
    if provenance_validator.as_path() != prepared.provenance_validator.as_path() {
        return Err("provenance validator resolved path changed after preparation".into());
    }
    let verifier = crate::repo_path("tools/agave-loopback-validator/verify-runtime.py");
    let output = Command::new("python3")
        .arg(verifier)
        .arg("--binary")
        .arg(&provenance_validator)
        .output()?;
    if output.stdout.len().saturating_add(output.stderr.len()) > 65_536 {
        return Err("validator provenance verifier output exceeded 65536 bytes".into());
    }
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(2_048)
            .collect::<String>();
        return Err(format!("pinned loopback validator verification failed: {detail}").into());
    }
    if solana_sha256_hasher::hash(&bounded_read_resolved(
        &provenance_validator,
        MAX_VALIDATOR_BYTES,
        "provenance validator binary",
    )?)
    .to_bytes()
        != prepared.validator_sha256
    {
        return Err("provenance validator digest changed after preparation".into());
    }
    Ok(())
}

fn verify_staged_executables(prepared: &PreparedLocalChain) -> Result<()> {
    for executable in &prepared.staged_executables {
        verify_file_digest(
            &executable.path,
            executable.expected_sha256,
            executable.maximum_bytes,
            executable.name,
        )?;
    }
    Ok(())
}

fn probe_loopback_listeners(
    prepared: &PreparedLocalChain,
    child: &mut Child,
) -> Result<()> {
    let probe = crate::repo_path("tools/agave-loopback-validator/probe-listeners.sh");
    let output = Command::new(probe)
        .arg(child.id().to_string())
        .arg(prepared.rpc_port.to_string())
        .arg(prepared.faucet_port.to_string())
        .arg(prepared.invocation.executable())
        .output()?;
    if output.stdout.len().saturating_add(output.stderr.len()) > 262_144 {
        return Err("loopback listener probe output exceeded 262144 bytes".into());
    }
    let evidence = [output.stdout, output.stderr].concat();
    write_new(
        &prepared.layout.root().join("listeners-before.txt"),
        &evidence,
    )?;
    if !output.status.success() {
        return Err("local validator listener isolation probe refused the process".into());
    }
    Ok(())
}

fn observe_local_genesis(child: &mut Child, rpc_http_url: &str) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getGenesisHash",
        "params": []
    })
    .to_string();
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!("local validator exited before genesis observation: {status}").into());
        }
        let output = Command::new("curl")
            .args([
                "-q",
                "--fail-with-body",
                "--silent",
                "--show-error",
                "--max-time",
                "1",
                "--connect-timeout",
                "1",
                "--max-filesize",
                "65536",
                "--max-redirs",
                "0",
                "--noproxy",
                "*",
                "--proxy",
                "",
                "--proto",
                "=http",
                "-H",
                "Content-Type: application/json",
                "-X",
                "POST",
                "--data-binary",
                &request,
                rpc_http_url,
            ])
            .output();
        if let Ok(output) = output {
            if output.status.success() && !output.stdout.is_empty() && output.stdout.len() <= 65_536 {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if value.get("jsonrpc").and_then(serde_json::Value::as_str) == Some("2.0")
                        && value.get("id").and_then(serde_json::Value::as_u64) == Some(1)
                    {
                        if let Some(genesis) = value
                            .get("result")
                            .and_then(serde_json::Value::as_str)
                        {
                            address(genesis, "observed local genesis hash")?;
                            return Ok(genesis.to_string());
                        }
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            return Err("local validator did not return a valid genesis hash within 30 seconds".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn seal_chain_config(prepared: &PreparedLocalChain, observed_genesis: &str) -> Result<()> {
    if prepared
        .expected_genesis_hash
        .as_deref()
        .is_some_and(|expected| expected != observed_genesis)
    {
        return Err("observed local genesis differs from the optional expected hash".into());
    }
    let chain = compose_chain_config::compose(&compose_chain_config::ComposeOptions {
        local_release_manifest: prepared.layout.public_manifest().to_path_buf(),
        capability_manifest: prepared.capability_manifest.clone(),
        cluster_name: "local-validator".to_string(),
        expected_genesis: observed_genesis.to_string(),
        rpc_http_url: prepared.rpc_http_url.clone(),
        rpc_websocket_url: prepared.rpc_websocket_url.clone(),
    })?;
    write_new(&prepared.chain_config, chain.as_bytes())
}

pub fn launch_and_serve(options: &LocalLaunchOptions) -> Result<()> {
    let prepared = prepare(options)?;
    verify_pinned_validator(&prepared)?;
    let stdout = open_new_log(&prepared.layout.root().join("validator.stdout.log"))?;
    let stderr = open_new_log(&prepared.layout.root().join("validator.stderr.log"))?;
    let mut command = Command::new(prepared.invocation.executable());
    command
        .args(prepared.invocation.arguments())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    verify_staged_executables(&prepared)?;
    let mut validator = ValidatorGuard(command.spawn()?);
    probe_loopback_listeners(&prepared, &mut validator.0)?;
    let observed_genesis = observe_local_genesis(&mut validator.0, &prepared.rpc_http_url)?;
    seal_chain_config(&prepared, &observed_genesis)?;
    chain_server::serve(options.server_port, options.statics.clone(), &prepared.chain_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_like_and_noncanonical_inputs_are_refused() {
        assert!(refuse_key_like_path(Path::new("/tmp/.solana/id.json"), "fixture").is_err());
        assert!(refuse_key_like_path(Path::new("/tmp/wallet.json"), "fixture").is_err());
        assert!(refuse_key_like_path(Path::new("/tmp/seed.json"), "fixture").is_err());
        assert!(refuse_key_like_path(Path::new("/tmp/mnemonic.json"), "fixture").is_err());
        assert!(refuse_key_like_path(Path::new("/tmp/secret.json"), "fixture").is_err());
        assert!(refuse_key_like_path(Path::new("relative.so"), "ELF").is_err());
        assert!(positive_u64("01", "slot").is_err());
        assert!(positive_u64("0", "slot").is_err());
        assert_eq!(local_synthesized_slot("0", "slot").unwrap(), 0);
        assert!(local_synthesized_slot("1", "slot").is_err());
        assert!(local_synthesized_slot("00", "slot").is_err());
        assert!(digest(&"00".repeat(32), "digest").is_err());
    }

    #[test]
    fn collection_aggregate_bounds_are_checked() {
        let mut total = MAX_EXTERNAL_PROGRAM_BYTES;
        assert!(add_bounded_size(
            &mut total,
            1,
            MAX_EXTERNAL_PROGRAM_BYTES,
            "external program"
        )
        .is_err());
        assert!(require_bounded_count(
            MAX_GENESIS_ACCOUNTS + 1,
            MAX_GENESIS_ACCOUNTS,
            "genesis_accounts"
        )
        .is_err());
    }

    #[test]
    fn resolved_key_paths_and_symlink_file_leaves_are_refused() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock is after the Unix epoch")
            .as_nanos();
        let temporary = std::fs::canonicalize(std::env::temp_dir())
            .expect("temporary directory has a canonical path");
        let base = temporary.join(format!(
            "dragons-clutch-input-alias-{}-{nonce}",
            std::process::id()
        ));
        let wallet_directory = base.join("wallet-vault");
        let directory_alias = base.join("ordinary-inputs");
        std::fs::create_dir(&base).expect("test base is fresh");
        std::fs::create_dir(&wallet_directory).expect("test target is fresh");
        let hidden_target = wallet_directory.join("release.json");
        std::fs::write(&hidden_target, b"not-wallet-material")
            .expect("test target is written");
        std::os::unix::fs::symlink(&wallet_directory, &directory_alias)
            .expect("test directory alias is created");
        assert!(
            resolve_existing_input(&directory_alias.join("release.json"), "input").is_err()
        );

        let ordinary_target = base.join("ordinary.json");
        let file_alias = base.join("input.json");
        std::fs::write(&ordinary_target, b"ordinary").expect("test file is written");
        std::os::unix::fs::symlink(&ordinary_target, &file_alias)
            .expect("test file alias is created");
        assert!(resolve_existing_input(&file_alias, "input").is_err());

        std::fs::remove_file(file_alias).expect("test file alias is removed");
        std::fs::remove_file(ordinary_target).expect("test file is removed");
        std::fs::remove_file(directory_alias).expect("test directory alias is removed");
        std::fs::remove_file(hidden_target).expect("test target is removed");
        std::fs::remove_dir(wallet_directory).expect("test target directory is removed");
        std::fs::remove_dir(base).expect("test base is removed");
    }

    #[test]
    fn source_acquisition_modes_are_disjoint() {
        assert!(acquisition(AcquisitionWire {
            mode: "pinned-local-capture".to_string(),
            capture_manifest_sha256: Some("11".repeat(32)),
            https_rpc_url: None,
            maximum_account_reads: None,
        })
        .is_ok());
        assert!(acquisition(AcquisitionWire {
            mode: "pinned-local-capture".to_string(),
            capture_manifest_sha256: Some("11".repeat(32)),
            https_rpc_url: Some("https://api.devnet.solana.com".to_string()),
            maximum_account_reads: Some("1".to_string()),
        })
        .is_err());
    }
}
