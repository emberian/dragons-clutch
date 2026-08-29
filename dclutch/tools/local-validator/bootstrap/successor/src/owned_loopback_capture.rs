//! Singular parser and Loader decoder for one finalized owned-loopback capture.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr as _,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_pyth_svm::{ProgramDataV3View, ProgramV3View};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use solana_sdk::{hash::Hash, pubkey::Pubkey};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Error, Result,
    cluster::{DEVNET_GENESIS_HASH, MAINNET_BETA_GENESIS_HASH},
    plan::hex,
    rpc::parse_json_without_duplicate_keys_v1,
};

pub(crate) const SCHEMA_V1: &str = "dclutch-owned-loopback-captured-finalized-rpc-v1";
const MAX_CAPTURE_BYTES_V1: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CaptureRefV1 {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) schema: String,
    pub(crate) finalized_slot: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CaptureV1 {
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
    pub(crate) genesis_hash: String,
    pub(crate) finalized_slot: u64,
    accounts: BTreeMap<String, CapturedAccountV1>,
}

#[derive(Clone, Debug)]
pub(crate) struct LoaderAccountV1 {
    pub(crate) address: Pubkey,
    pub(crate) context_slot: u64,
    pub(crate) lamports: u64,
    pub(crate) owner: Pubkey,
    pub(crate) executable: bool,
    pub(crate) rent_epoch: u64,
    pub(crate) data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct LoaderPairV1 {
    pub(crate) role: String,
    pub(crate) program: LoaderAccountV1,
    pub(crate) program_data: LoaderAccountV1,
    pub(crate) deployment_slot: u64,
    pub(crate) elf_sha256: String,
    pub(crate) program_data_sha256: String,
    pub(crate) upgrade_authority: Option<Pubkey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeploymentSlotPolicyV1 {
    Nonzero,
    ExactZeroImmutable,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureDocumentV1 {
    schema: String,
    #[serde(rename = "genesisHash")]
    genesis_hash: String,
    commitment: String,
    #[serde(rename = "finalizedSlot")]
    finalized_slot: String,
    transactions: Value,
    accounts: BTreeMap<String, CapturedAccountV1>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CapturedAccountV1 {
    context_slot: String,
    value: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RpcAccountV1 {
    lamports: u64,
    owner: String,
    data: [String; 2],
    executable: bool,
    rent_epoch: u64,
    #[serde(default)]
    space: Option<u64>,
}

pub(crate) fn authenticate_v1(path: &Path) -> Result<CaptureV1> {
    let path = canonical_regular_v1(path, "finalized owned-loopback capture")?;
    let metadata = fs::metadata(&path)?;
    if metadata.len() == 0 || metadata.len() > MAX_CAPTURE_BYTES_V1 {
        return Err(Error::new(
            "finalized owned-loopback capture must contain one through 32 MiB",
        ));
    }
    let bytes = fs::read(&path)?;
    let value = parse_json_without_duplicate_keys_v1(&bytes)
        .map_err(|error| Error::new(format!("finalized owned-loopback capture: {error}")))?;
    let document: CaptureDocumentV1 = serde_json::from_value(value)?;
    if document.schema != SCHEMA_V1
        || document.commitment != "finalized"
        || !document.transactions.is_object()
    {
        return Err(Error::new(
            "capture is not the exact finalized owned-loopback envelope",
        ));
    }
    Hash::from_str(&document.genesis_hash)
        .map_err(|error| Error::new(format!("capture genesis hash: {error}")))?;
    if document.genesis_hash == DEVNET_GENESIS_HASH
        || document.genesis_hash == MAINNET_BETA_GENESIS_HASH
    {
        return Err(Error::new("capture names a public cluster"));
    }
    let finalized_slot = canonical_decimal_v1(&document.finalized_slot, "capture finalized slot")?;
    if finalized_slot == 0 {
        return Err(Error::new("capture finalized slot must be positive"));
    }
    if document.accounts.is_empty() {
        return Err(Error::new("capture account map is empty"));
    }
    for (address, row) in &document.accounts {
        let key = Pubkey::from_str(address)
            .map_err(|error| Error::new(format!("capture account address: {error}")))?;
        if key.to_string() != *address
            || canonical_decimal_v1(&row.context_slot, "capture account context slot")?
                != finalized_slot
        {
            return Err(Error::new(
                "capture account key or context slot differs from the singular finalized context",
            ));
        }
    }
    Ok(CaptureV1 {
        path,
        sha256: sha256_v1(&bytes),
        genesis_hash: document.genesis_hash,
        finalized_slot,
        accounts: document.accounts,
    })
}

impl CaptureV1 {
    pub(crate) fn reference_v1(&self) -> Result<CaptureRefV1> {
        Ok(CaptureRefV1 {
            path: self
                .path
                .to_str()
                .ok_or_else(|| Error::new("capture path is not UTF-8"))?
                .to_owned(),
            sha256: self.sha256.clone(),
            schema: SCHEMA_V1.into(),
            finalized_slot: self.finalized_slot.to_string(),
        })
    }

    pub(crate) fn loader_pair_v1(
        &self,
        role: &str,
        program_id: Pubkey,
        expected_program_data: Option<Pubkey>,
        slot_policy: DeploymentSlotPolicyV1,
    ) -> Result<LoaderPairV1> {
        let program = self.loader_account_v1(role, "Program", program_id, true)?;
        let program_view = ProgramV3View::parse(&program.data)
            .map_err(|error| Error::new(format!("captured {role} Program: {error:?}")))?;
        let program_data_id = Pubkey::new_from_array(program_view.programdata());
        if expected_program_data.is_some_and(|expected| expected != program_data_id) {
            return Err(Error::new(format!(
                "captured {role} ProgramData link differs from its authenticated source"
            )));
        }
        let program_data = self.loader_account_v1(role, "ProgramData", program_data_id, false)?;
        let view = ProgramDataV3View::parse(&program_data.data)
            .map_err(|error| Error::new(format!("captured {role} ProgramData: {error:?}")))?;
        let deployment_slot = view.deployment_slot();
        if program.context_slot < deployment_slot
            || program_data.context_slot < deployment_slot
            || program.context_slot > self.finalized_slot
            || program_data.context_slot > self.finalized_slot
        {
            return Err(Error::new(format!(
                "captured {role} Loader pair is outside its finalized boundary"
            )));
        }
        let upgrade_authority = view.upgrade_authority().map(Pubkey::new_from_array);
        match slot_policy {
            DeploymentSlotPolicyV1::Nonzero if deployment_slot == 0 => {
                return Err(Error::new(format!(
                    "captured {role} deployment slot must be nonzero"
                )));
            }
            DeploymentSlotPolicyV1::ExactZeroImmutable
                if deployment_slot != 0
                    || upgrade_authority.is_some()
                    || program_data.data.get(12) != Some(&0)
                    || !program_data
                        .data
                        .get(13..45)
                        .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0)) =>
            {
                return Err(Error::new(format!(
                    "captured {role} provider is not exact slot-zero tag-None ProgramData"
                )));
            }
            _ => {}
        }
        Ok(LoaderPairV1 {
            role: role.into(),
            program,
            program_data_sha256: sha256_v1(&program_data.data),
            elf_sha256: sha256_v1(view.elf()),
            program_data,
            deployment_slot,
            upgrade_authority,
        })
    }

    pub(crate) fn loader_addresses_v1(&self) -> Result<BTreeSet<String>> {
        let loader = bpf_loader_upgradeable::ID.to_string();
        let mut addresses = BTreeSet::new();
        for (address, row) in &self.accounts {
            if row.value.get("owner").and_then(Value::as_str) == Some(loader.as_str()) {
                let _: RpcAccountV1 =
                    serde_json::from_value(row.value.clone()).map_err(|error| {
                        Error::new(format!("captured Loader account {address}: {error}"))
                    })?;
                addresses.insert(address.clone());
            }
        }
        Ok(addresses)
    }

    fn loader_account_v1(
        &self,
        role: &str,
        kind: &str,
        address: Pubkey,
        executable: bool,
    ) -> Result<LoaderAccountV1> {
        let row = self
            .accounts
            .get(&address.to_string())
            .ok_or_else(|| Error::new(format!("capture omitted {role} {kind} account")))?;
        let context_slot = canonical_decimal_v1(
            &row.context_slot,
            &format!("captured {role} {kind} context slot"),
        )?;
        let account: RpcAccountV1 = serde_json::from_value(row.value.clone())?;
        let owner = Pubkey::from_str(&account.owner)
            .map_err(|error| Error::new(format!("captured {role} {kind} owner: {error}")))?;
        if owner != bpf_loader_upgradeable::ID
            || account.executable != executable
            || account.lamports == 0
            || account.data[1] != "base64"
        {
            return Err(Error::new(format!(
                "captured {role} {kind} owner, privilege, funding, or encoding differs"
            )));
        }
        let data = BASE64
            .decode(&account.data[0])
            .map_err(|error| Error::new(format!("captured {role} {kind} base64: {error}")))?;
        if BASE64.encode(&data) != account.data[0]
            || account
                .space
                .is_some_and(|space| space != data.len() as u64)
        {
            return Err(Error::new(format!(
                "captured {role} {kind} data or declared space is noncanonical"
            )));
        }
        Ok(LoaderAccountV1 {
            address,
            context_slot,
            lamports: account.lamports,
            owner,
            executable: account.executable,
            rent_epoch: account.rent_epoch,
            data,
        })
    }
}

fn canonical_regular_v1(path: &Path, label: &str) -> Result<PathBuf> {
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

fn canonical_decimal_v1(value: &str, label: &str) -> Result<u64> {
    let parsed = value
        .parse::<u64>()
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    if parsed.to_string() != value {
        return Err(Error::new(format!("{label} is not canonical decimal")));
    }
    Ok(parsed)
}

fn sha256_v1(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::json;
    use solana_sdk_ids::system_program;

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Root(PathBuf);

    impl Root {
        fn new() -> Self {
            let root = fs::canonicalize(std::env::temp_dir())
                .expect("canonical temp")
                .join(format!(
                    "dclutch-capture-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            fs::create_dir(&root).expect("create test root");
            Self(root)
        }

        fn write(&self, name: &str, value: &Value) -> PathBuf {
            let path = self.0.join(name);
            let mut bytes = serde_json::to_vec_pretty(value).expect("capture JSON");
            bytes.push(b'\n');
            fs::write(&path, bytes).expect("write capture");
            path
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove scoped test root");
        }
    }

    fn fixture() -> (Pubkey, Pubkey, Vec<u8>, Vec<u8>, Value) {
        let program = Pubkey::new_unique();
        let program_data =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let mut program_bytes = vec![0_u8; 36];
        program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(program_data.as_ref());
        let mut program_data_bytes = vec![0_u8; 45];
        program_data_bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
        program_data_bytes.extend_from_slice(b"\x7fELFprovider");
        let account = |data: &[u8], executable: bool| {
            json!({
                "lamports": 1,
                "owner": bpf_loader_upgradeable::ID.to_string(),
                "data": [BASE64.encode(data), "base64"],
                "executable": executable,
                "rentEpoch": 0,
                "space": data.len(),
            })
        };
        let capture = json!({
            "schema": SCHEMA_V1,
            "genesisHash": Pubkey::new_unique().to_string(),
            "commitment": "finalized",
            "finalizedSlot": "10",
            "transactions": {},
            "accounts": {
                program.to_string(): {"contextSlot":"10", "value":account(&program_bytes, true)},
                program_data.to_string(): {"contextSlot":"10", "value":account(&program_data_bytes, false)},
            }
        });
        (
            program,
            program_data,
            program_bytes,
            program_data_bytes,
            capture,
        )
    }

    fn replace_data(capture: &mut Value, address: Pubkey, bytes: &[u8]) {
        capture["accounts"][address.to_string()]["value"]["data"][0] =
            Value::String(BASE64.encode(bytes));
        capture["accounts"][address.to_string()]["value"]["space"] = json!(bytes.len());
    }

    #[test]
    fn exact_capture_authenticates_slot_zero_immutable_loader_pair() {
        let root = Root::new();
        let (program, program_data, _, _, capture) = fixture();
        let path = root.write("capture.json", &capture);
        let checked = authenticate_v1(&path).expect("capture");
        let pair = checked
            .loader_pair_v1(
                "provider",
                program,
                Some(program_data),
                DeploymentSlotPolicyV1::ExactZeroImmutable,
            )
            .expect("immutable provider");
        assert_eq!(pair.deployment_slot, 0);
        assert_eq!(pair.upgrade_authority, None);
        assert_eq!(
            checked.reference_v1().expect("reference").finalized_slot,
            "10"
        );
    }

    #[test]
    fn capture_refuses_tag_slot_link_elf_full_and_context_substitution() {
        let root = Root::new();
        let (program, program_data, program_bytes, program_data_bytes, capture) = fixture();
        for (index, mutation) in ["tag", "slot", "link", "full", "context"]
            .into_iter()
            .enumerate()
        {
            let mut hostile = capture.clone();
            match mutation {
                "tag" => {
                    let mut bytes = program_data_bytes.clone();
                    bytes[12] = 1;
                    bytes[13..45].copy_from_slice(system_program::ID.as_ref());
                    replace_data(&mut hostile, program_data, &bytes);
                }
                "slot" => {
                    let mut bytes = program_data_bytes.clone();
                    bytes[4] = 1;
                    replace_data(&mut hostile, program_data, &bytes);
                }
                "link" => {
                    let mut bytes = program_bytes.clone();
                    bytes[4] ^= 1;
                    replace_data(&mut hostile, program, &bytes);
                }
                "full" => {
                    let mut bytes = program_data_bytes.clone();
                    bytes[13] = 1;
                    replace_data(&mut hostile, program_data, &bytes);
                }
                "context" => hostile["accounts"][program.to_string()]["contextSlot"] = json!("9"),
                _ => unreachable!(),
            }
            let path = root.write(&format!("hostile-{index}.json"), &hostile);
            let result = authenticate_v1(&path).and_then(|checked| {
                checked.loader_pair_v1(
                    "provider",
                    program,
                    Some(program_data),
                    DeploymentSlotPolicyV1::ExactZeroImmutable,
                )
            });
            assert!(result.is_err(), "{mutation} substitution was accepted");
        }
    }

    #[test]
    fn capture_refuses_public_genesis_noncanonical_slot_and_duplicate_json() {
        let root = Root::new();
        let (_, _, _, _, capture) = fixture();
        let mut public = capture.clone();
        public["genesisHash"] = json!(DEVNET_GENESIS_HASH);
        assert!(authenticate_v1(&root.write("public.json", &public)).is_err());
        let mut slot = capture;
        slot["finalizedSlot"] = json!("010");
        assert!(authenticate_v1(&root.write("slot.json", &slot)).is_err());
        let duplicate = root.0.join("duplicate.json");
        fs::write(
            &duplicate,
            br#"{"schema":"a","schema":"b","genesisHash":"x","commitment":"finalized","finalizedSlot":"1","transactions":{},"accounts":{}}"#,
        )
        .expect("duplicate capture");
        assert!(authenticate_v1(&duplicate).is_err());
    }
}
