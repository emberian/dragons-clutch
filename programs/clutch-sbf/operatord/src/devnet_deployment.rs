//! Checked, read-only devnet deployment-manifest composition.
//!
//! A devnet deployment is not a local-validator session.  This module owns a
//! separate canonical manifest shape and refuses the local-session schema,
//! marker, and paths.  It only composes the configuration consumed by
//! `chain-serve`; it has no wallet, signer, blockhash, faucet, deployment, or
//! transaction-submission capability.

use crate::compose_chain_config::{checked_capability_release, compose_checked_chain_config};
use crate::Result;
use clutch_local_real_pyth::rpc_index::ReleaseCoordinateLocusV2;
use serde::{Deserialize, Serialize};
use solana_address::Address;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

pub const DEVNET_DEPLOYMENT_MANIFEST_SCHEMA: &str =
    "dragons-clutch/devnet-deployment-manifest/v2";
pub const DEVNET_GENESIS_HASH: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
pub const DEVNET_RPC_HTTP: &str = "https://api.devnet.solana.com";
pub const DEVNET_RPC_WEBSOCKET: &str = "wss://api.devnet.solana.com/";
const MAX_DEPLOYMENT_MANIFEST_BYTES: usize = 65_536;
const MAX_ELF_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeDevnetOptions {
    pub deployment_manifest: PathBuf,
    pub capability_manifest: PathBuf,
    pub built_elf: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DevnetDeploymentManifestV2 {
    schema: String,
    network: String,
    genesis_hash: String,
    rpc_http_url: String,
    rpc_websocket_url: String,
    release_coordinates: String,
    program_id: String,
    program_data: String,
    program_data_sha256: String,
    deployment_slot: String,
    elf_sha256: String,
    capability_manifest_sha256: String,
    capability_profile_identity: String,
    source_commit: String,
    compiler_release_sha256: String,
    source_neutral_sink: String,
    signing: String,
    submission: String,
    deployment: String,
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

fn canonical_positive(text: &str, name: &str) -> Result<u64> {
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

fn parse_manifest(path: &Path) -> Result<(DevnetDeploymentManifestV2, [u8; 32])> {
    let bytes = bounded_read(
        path,
        MAX_DEPLOYMENT_MANIFEST_BYTES,
        "devnet deployment manifest",
    )?;
    if !bytes.is_ascii() {
        return Err("devnet deployment manifest must be ASCII".into());
    }
    let manifest: DevnetDeploymentManifestV2 = serde_json::from_slice(&bytes)?;
    let mut canonical = serde_json::to_vec(&manifest)?;
    canonical.push(b'\n');
    if bytes != canonical {
        return Err("devnet deployment manifest is not canonical compact JSON plus newline".into());
    }
    if manifest.schema != DEVNET_DEPLOYMENT_MANIFEST_SCHEMA
        || manifest.network != "solana-devnet"
        || manifest.genesis_hash != DEVNET_GENESIS_HASH
        || manifest.rpc_http_url != DEVNET_RPC_HTTP
        || manifest.rpc_websocket_url != DEVNET_RPC_WEBSOCKET
        || manifest.release_coordinates != "observed-finalized"
        || manifest.signing != "not-exposed"
        || manifest.submission != "not-exposed"
        || manifest.deployment != "not-exposed"
    {
        return Err("devnet deployment manifest is unsealed or authority-bearing".into());
    }
    let digest = solana_sha256_hasher::hash(&canonical).to_bytes();
    Ok((manifest, digest))
}

pub fn compose(options: &ComposeDevnetOptions) -> Result<String> {
    if options
        .deployment_manifest
        .extension()
        .and_then(|value| value.to_str())
        != Some("json")
        || options
            .capability_manifest
            .extension()
            .and_then(|value| value.to_str())
            != Some("json")
    {
        return Err("devnet deployment and capability inputs must be explicit .json files".into());
    }
    let deployment_manifest =
        resolve_existing_input(&options.deployment_manifest, "devnet deployment manifest")?;
    let capability_manifest =
        resolve_existing_input(&options.capability_manifest, "capability manifest")?;
    let built_elf_path = resolve_existing_input(&options.built_elf, "built release ELF")?;
    if built_elf_path.extension().and_then(|value| value.to_str()) != Some("so") {
        return Err("built release ELF must be an absolute .so path".into());
    }
    let (manifest, deployment_manifest_sha256) =
        parse_manifest(&deployment_manifest)?;
    let checked = checked_capability_release(&capability_manifest)?;
    if hash32(
        &manifest.capability_manifest_sha256,
        "capability_manifest_sha256",
    )? != checked.manifest_sha256
        || hash32(
            &manifest.capability_profile_identity,
            "capability_profile_identity",
        )? != checked.profile_identity
        || hash32(&manifest.elf_sha256, "elf_sha256")? != checked.elf_sha256
        || manifest.source_commit != checked.source_commit.as_str()
    {
        return Err("devnet deployment coordinates disagree with the checked release".into());
    }
    hash32(
        &manifest.compiler_release_sha256,
        "compiler_release_sha256",
    )?;
    let built_elf = bounded_read_resolved(&built_elf_path, MAX_ELF_BYTES, "built release ELF")?;
    if solana_sha256_hasher::hash(&built_elf).to_bytes() != checked.elf_sha256 {
        return Err("built release ELF differs from the checked devnet deployment".into());
    }
    let program = Address::from_str(&manifest.program_id)?;
    let program_data = Address::from_str(&manifest.program_data)?;
    let program_data_sha256 = hash32(&manifest.program_data_sha256, "program_data_sha256")?;
    let slot = canonical_positive(&manifest.deployment_slot, "deployment_slot")?;
    compose_checked_chain_config(
        &checked,
        "solana-devnet",
        DEVNET_GENESIS_HASH,
        DEVNET_RPC_HTTP,
        DEVNET_RPC_WEBSOCKET,
        program,
        program_data,
        program_data_sha256,
        slot,
        ReleaseCoordinateLocusV2::ObservedPositive,
        &manifest.source_neutral_sink,
        &manifest.compiler_release_sha256,
        deployment_manifest_sha256,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> DevnetDeploymentManifestV2 {
        DevnetDeploymentManifestV2 {
            schema: DEVNET_DEPLOYMENT_MANIFEST_SCHEMA.to_string(),
            network: "solana-devnet".to_string(),
            genesis_hash: DEVNET_GENESIS_HASH.to_string(),
            rpc_http_url: DEVNET_RPC_HTTP.to_string(),
            rpc_websocket_url: DEVNET_RPC_WEBSOCKET.to_string(),
            release_coordinates: "observed-finalized".to_string(),
            program_id: "11111111111111111111111111111112".to_string(),
            program_data: "11111111111111111111111111111113".to_string(),
            program_data_sha256: "66".repeat(32),
            deployment_slot: "7".to_string(),
            elf_sha256: "11".repeat(32),
            capability_manifest_sha256: "22".repeat(32),
            capability_profile_identity: "33".repeat(32),
            source_commit: "44".repeat(20),
            compiler_release_sha256: "55".repeat(32),
            source_neutral_sink: "11111111111111111111111111111114".to_string(),
            signing: "not-exposed".to_string(),
            submission: "not-exposed".to_string(),
            deployment: "not-exposed".to_string(),
        }
    }

    #[test]
    fn devnet_manifest_has_no_local_or_authority_fields() -> Result<()> {
        let bytes = serde_json::to_vec(&manifest())?;
        let text = std::str::from_utf8(&bytes)?;
        assert!(!text.contains("session"));
        assert!(!text.contains("wallet"));
        assert!(!text.contains("keypair"));
        assert!(serde_json::from_slice::<DevnetDeploymentManifestV2>(
            br#"{"schema":"dragons-clutch/local-validator-public-manifest/v7"}"#,
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn devnet_coordinates_are_exact() {
        let mut wrong = manifest();
        wrong.rpc_http_url = "http://127.0.0.1:8899".to_string();
        assert_ne!(wrong.rpc_http_url, DEVNET_RPC_HTTP);
        assert!(canonical_positive("01", "slot").is_err());
        assert!(canonical_positive("0", "slot").is_err());
        assert!(refuse_key_like_path(Path::new("/tmp/wallet.json"), "manifest").is_err());
        assert!(refuse_key_like_path(Path::new("/tmp/seed.json"), "manifest").is_err());
        assert!(refuse_key_like_path(Path::new("/tmp/mnemonic.json"), "manifest").is_err());
        assert!(refuse_key_like_path(Path::new("/tmp/secret.json"), "manifest").is_err());
    }

    #[test]
    fn devnet_inputs_refuse_resolved_secret_paths_and_symlink_leaves() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock is after the Unix epoch")
            .as_nanos();
        let temporary = std::fs::canonicalize(std::env::temp_dir())
            .expect("temporary directory has a canonical path");
        let base = temporary.join(format!(
            "dragons-clutch-devnet-alias-{}-{nonce}",
            std::process::id()
        ));
        let secret_directory = base.join("secret-store");
        let directory_alias = base.join("ordinary-inputs");
        std::fs::create_dir(&base).expect("test base is fresh");
        std::fs::create_dir(&secret_directory).expect("test target is fresh");
        let hidden_target = secret_directory.join("manifest.json");
        std::fs::write(&hidden_target, b"not-secret-material").expect("test target is written");
        std::os::unix::fs::symlink(&secret_directory, &directory_alias)
            .expect("test directory alias is created");
        assert!(
            resolve_existing_input(&directory_alias.join("manifest.json"), "input").is_err()
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
        std::fs::remove_dir(secret_directory).expect("test target directory is removed");
        std::fs::remove_dir(base).expect("test base is removed");
    }
}
