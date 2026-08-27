//! The mainnet twin: a loopback validator carrying the synthetic-of-real DBC
//! world for the relayer daemon to observe.
//!
//! **What is real and what is invented, field by field.** The venue program
//! and ProgramData ADDRESSES are Meteora's real mainnet ones
//! (`dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN` /
//! `HUfnSSiJxgspQm6C1rkqv6L3XgVtn7AESApgCQpCXCYh`), and the `VirtualPool` BYTE
//! LAYOUT is the real one from the published source (§10.1: 424 bytes,
//! discriminator `d5e005d1 6245775c`, `migration_progress` at 308,
//! `is_migrated` at 305, `finish_curve_timestamp` at 344). The ELF tail, the
//! deployment slot, the upgrade authority, and the pool's address and values
//! are SYNTHETIC: the CS dossier never read them from mainnet
//! (`CHAIN_STATE_SOURCES_2026_08.md` §7 and `MAINNET_STATE_RELAY.md` §12.8
//! item 1 both say so), and this lane makes no public reads. Everything the
//! campaign derives from these facts inherits the synthetic-of-real label.

use std::io::Write as _;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;

use crate::{Error, Result, rpc::Rpc};

/// `dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN` (real mainnet address).
pub(crate) const DBC_PROGRAM: [u8; 32] = [
    0x09, 0x60, 0x0c, 0xa5, 0x24, 0xf7, 0xb1, 0xb7, 0xd6, 0xcc, 0xb1, 0xc3, 0x97, 0x3a, 0xa0, 0x33,
    0x0d, 0x19, 0x03, 0xda, 0x60, 0x1c, 0xc9, 0xb5, 0xde, 0xe3, 0xc6, 0x62, 0xb4, 0xca, 0xd1, 0x49,
];
/// `HUfnSSiJxgspQm6C1rkqv6L3XgVtn7AESApgCQpCXCYh` (real mainnet address).
pub(crate) const DBC_PROGRAMDATA: [u8; 32] = [
    0xf4, 0xd1, 0x86, 0x75, 0x30, 0x52, 0x43, 0xdc, 0x37, 0x9e, 0xb4, 0x94, 0x57, 0xaf, 0xa7, 0xdd,
    0x60, 0x00, 0x24, 0x63, 0xdc, 0xdc, 0x6f, 0x11, 0xb2, 0x68, 0x5d, 0x23, 0x34, 0x9c, 0xfc, 0xba,
];
/// The BPF upgradeable loader.
pub(crate) const LOADER_V3: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

/// A synthetic deployment slot. The dossier never read the real one; the
/// venue release and this twin state the SAME number, which is what the
/// consumption authenticates. Labeled synthetic wherever it appears.
pub(crate) const SYNTHETIC_DEPLOYMENT_SLOT: u64 = 423_941_138;

pub(crate) const VIRTUAL_POOL_DISCRIMINATOR: [u8; 8] =
    [0xd5, 0xe0, 0x05, 0xd1, 0x62, 0x45, 0x77, 0x5c];
pub(crate) const VIRTUAL_POOL_BYTES: usize = 424;
const MIGRATION_PROGRESS_OFFSET: usize = 308;
const IS_MIGRATED_OFFSET: usize = 305;
const FINISH_CURVE_TIMESTAMP_OFFSET: usize = 344;
pub(crate) const MIGRATION_PROGRESS_CREATED_POOL: u8 = 3;
/// Width of a synthetic ELF tail: enough for several daemon body pages
/// without being a megabyte of noise in the fixture directory.
const SYNTHETIC_ELF_TAIL_BYTES: usize = 4_096;

fn domain_bytes(role: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/relayed-vertical/twin/v1");
    hasher.update([0]);
    hasher.update(role.as_bytes());
    hasher.finalize().into()
}

/// The synthetic pool address: stable, off any real chain, labeled synthetic.
pub(crate) fn pool_address() -> [u8; 32] {
    domain_bytes("virtual-pool-address")
}

/// The synthetic upgrade authority the twin's ProgramData claims.
pub(crate) fn upgrade_authority() -> [u8; 32] {
    domain_bytes("venue-upgrade-authority")
}

/// The deterministic synthetic ELF tail and its digest.
pub(crate) fn synthetic_elf_tail() -> Vec<u8> {
    let mut tail = Vec::with_capacity(SYNTHETIC_ELF_TAIL_BYTES);
    let mut counter: u32 = 0;
    while tail.len() < SYNTHETIC_ELF_TAIL_BYTES {
        let mut hasher = Sha256::new();
        hasher.update(b"dclutch/relayed-vertical/twin/elf-tail/v1");
        hasher.update(counter.to_le_bytes());
        tail.extend_from_slice(&hasher.finalize());
        counter = counter.wrapping_add(1);
    }
    tail.truncate(SYNTHETIC_ELF_TAIL_BYTES);
    tail
}

pub(crate) fn synthetic_elf_digest() -> [u8; 32] {
    Sha256::digest(synthetic_elf_tail()).into()
}

/// The 36-byte Loader V3 `Program` account body pointing at the ProgramData.
fn program_body() -> Vec<u8> {
    let mut data = vec![0u8; 36];
    data[..4].copy_from_slice(&2u32.to_le_bytes());
    data[4..36].copy_from_slice(&DBC_PROGRAMDATA);
    data
}

/// The ProgramData body: the 45-byte Loader V3 prefix plus the synthetic tail.
fn programdata_body() -> Vec<u8> {
    let tail = synthetic_elf_tail();
    let mut data = vec![0u8; 45 + tail.len()];
    data[..4].copy_from_slice(&3u32.to_le_bytes());
    data[4..12].copy_from_slice(&SYNTHETIC_DEPLOYMENT_SLOT.to_le_bytes());
    data[12] = 1;
    data[13..45].copy_from_slice(&upgrade_authority());
    data[45..].copy_from_slice(&tail);
    data
}

/// A graduated pool: `migration_progress = CreatedPool`, `is_migrated = 1`,
/// and a nonzero `finish_curve_timestamp`. Layout real, values invented.
fn graduated_pool_body(finish_unix_seconds: i64) -> Vec<u8> {
    let mut data = vec![0u8; VIRTUAL_POOL_BYTES];
    data[..8].copy_from_slice(&VIRTUAL_POOL_DISCRIMINATOR);
    data[MIGRATION_PROGRESS_OFFSET] = MIGRATION_PROGRESS_CREATED_POOL;
    data[IS_MIGRATED_OFFSET] = 1;
    let stamp = u64::try_from(finish_unix_seconds.max(1)).unwrap_or(1);
    data[FINISH_CURVE_TIMESTAMP_OFFSET..FINISH_CURVE_TIMESTAMP_OFFSET + 8]
        .copy_from_slice(&stamp.to_le_bytes());
    data
}

/// One `--account` fixture file in the CLI's JSON shape.
fn write_account_fixture(
    directory: &Path,
    address: Pubkey,
    owner: &str,
    data: &[u8],
    executable: bool,
) -> Result<PathBuf> {
    // Comfortably rent-exempt for the width; the twin is not a rent study.
    let lamports = 1_000_000_000u64.max(u64::try_from(data.len()).unwrap_or(0) * 10_000);
    let value = serde_json::json!({
        "pubkey": address.to_string(),
        "account": {
            "lamports": lamports,
            "data": [base64::engine::general_purpose::STANDARD.encode(data), "base64"],
            "owner": owner,
            "executable": executable,
            "rentEpoch": 0u64,
            "space": data.len(),
        }
    });
    let path = directory.join(format!("{address}.json"));
    let mut file = std::fs::File::create(&path)?;
    file.write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
    Ok(path)
}

/// Bind a whole port block on 127.0.0.1 to prove it free, then release it.
fn allocate_port_block() -> Result<u16> {
    // The successor launcher derives {base, base+2, base+3, base+10..base+41};
    // the twin uses the same shape so the two never interleave by accident.
    for candidate in (24_000..48_000u16).step_by(64) {
        let mut held = Vec::new();
        let mut all = true;
        for offset in [0u16, 1, 2, 3].into_iter().chain(10..42) {
            match TcpListener::bind(("127.0.0.1", candidate + offset)) {
                Ok(listener) => held.push(listener),
                Err(_) => {
                    all = false;
                    break;
                }
            }
        }
        drop(held);
        if all {
            return Ok(candidate);
        }
    }
    Err(Error::new("no free port block for the mainnet twin"))
}

/// The running twin and everything the campaign derived from it.
pub(crate) struct MainnetTwinV1 {
    child: Child,
    pub(crate) rpc_url: String,
    /// The twin's real genesis hash, read over RPC after boot.
    pub(crate) genesis_hash_base58: String,
    pub(crate) pool: Pubkey,
    #[allow(dead_code)]
    pub(crate) ledger: PathBuf,
}

impl Drop for MainnetTwinV1 {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start the twin with the synthetic DBC world installed at genesis.
pub(crate) fn start(work: &Path, finish_unix_seconds: i64) -> Result<MainnetTwinV1> {
    let fixtures = work.join("twin-fixtures");
    let ledger = work.join("twin-ledger");
    std::fs::create_dir_all(&fixtures)?;
    let program = Pubkey::new_from_array(DBC_PROGRAM);
    let programdata = Pubkey::new_from_array(DBC_PROGRAMDATA);
    let pool = Pubkey::new_from_array(pool_address());
    let program_fixture =
        write_account_fixture(&fixtures, program, LOADER_V3, &program_body(), true)?;
    let programdata_fixture = write_account_fixture(
        &fixtures,
        programdata,
        LOADER_V3,
        &programdata_body(),
        false,
    )?;
    let pool_fixture = write_account_fixture(
        &fixtures,
        pool,
        &program.to_string(),
        &graduated_pool_body(finish_unix_seconds),
        false,
    )?;

    let base = allocate_port_block()?;
    let rpc_url = format!("http://127.0.0.1:{base}/");
    let log = std::fs::File::create(work.join("twin-validator.log"))?;
    let child = Command::new("solana-test-validator")
        .arg("--reset")
        .arg("--quiet")
        .arg("--bind-address")
        .arg("127.0.0.1")
        .arg("--rpc-port")
        .arg(base.to_string())
        .arg("--faucet-port")
        .arg((base + 2).to_string())
        .arg("--gossip-port")
        .arg((base + 3).to_string())
        .arg("--dynamic-port-range")
        .arg(format!("{}-{}", base + 10, base + 41))
        .arg("--ledger")
        .arg(&ledger)
        .arg("--account")
        .arg(program.to_string())
        .arg(&program_fixture)
        .arg("--account")
        .arg(programdata.to_string())
        .arg(&programdata_fixture)
        .arg("--account")
        .arg(pool.to_string())
        .arg(&pool_fixture)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|error| Error::new(format!("could not start the mainnet twin: {error}")))?;
    let mut twin = MainnetTwinV1 {
        child,
        rpc_url: rpc_url.clone(),
        genesis_hash_base58: String::new(),
        pool,
        ledger,
    };

    // Wait for health, then read the genesis hash the daemon config will pin.
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if Instant::now() > deadline {
            return Err(Error::new(
                "the mainnet twin did not become healthy within 90 seconds",
            ));
        }
        if let Some(status) = twin
            .child
            .try_wait()
            .map_err(|error| Error::new(format!("twin wait: {error}")))?
        {
            return Err(Error::new(format!(
                "the mainnet twin exited during startup: {status}"
            )));
        }
        if let Ok(mut probe) = Rpc::connect(&rpc_url)
            && let Ok(value) = probe.call("getGenesisHash", &serde_json::json!([]))
            && let Some(hash) = value.as_str()
        {
            twin.genesis_hash_base58 = hash.to_owned();
            // The fixture accounts must actually be present before anyone
            // observes them; a twin without its world is a silent lie.
            let pool_present = probe
                .account(twin.pool)
                .ok()
                .flatten()
                .is_some_and(|account| account.data.len() == VIRTUAL_POOL_BYTES);
            if pool_present {
                return Ok(twin);
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
