//! Fresh test-only keys, the non-production ELF, and the local validator.
//!
//! This is `scripts/run_general_committed.sh`'s prologue, in process and in
//! the same order: mint keys into a private temporary directory, build the
//! explicitly different mock-source ELF, hash it, emit the plan from the
//! public keys only, then start a fresh ledger with the ELF loaded and the
//! plan's genesis accounts installed.
//!
//! The secrets this daemon creates are exactly the eight files below, and
//! they are unlinked when [`Keys`] drops.  Nothing here reads Solana CLI
//! wallet configuration, and no secret byte is published to the bus.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The signing roster the general-clearing plan needs: a fee payer, the
/// founding actor, three trading owners, and each trader's ordinary
/// Token-2022 collateral identity.
pub const KEY_NAMES: [&str; 8] = [
    "payer",
    "actor",
    "owner-b",
    "owner-c",
    "owner-d",
    "owner-b-collateral-token",
    "owner-c-collateral-token",
    "owner-d-collateral-token",
];

/// Which plan input each generated key is passed to the emitter as.
const KEY_VARIABLES: [(&str, &str); 8] = [
    ("payer", "CLUTCH_COMMITTED_PAYER"),
    ("actor", "CLUTCH_COMMITTED_ACTOR"),
    ("owner-b", "CLUTCH_COMMITTED_HOLDER"),
    ("owner-c", "CLUTCH_COMMITTED_TRADER_C"),
    ("owner-d", "CLUTCH_COMMITTED_TRADER_D"),
    (
        "owner-b-collateral-token",
        "CLUTCH_COMMITTED_HOLDER_COLLATERAL_TOKEN",
    ),
    (
        "owner-c-collateral-token",
        "CLUTCH_COMMITTED_TRADER_C_COLLATERAL_TOKEN",
    ),
    (
        "owner-d-collateral-token",
        "CLUTCH_COMMITTED_TRADER_D_COLLATERAL_TOKEN",
    ),
];

fn tool(variable: &str, name: &str) -> PathBuf {
    if let Ok(path) = env::var(variable) {
        return PathBuf::from(path);
    }
    let home = env::var("SOLANA_HOME").map_or_else(
        |_| {
            let base = env::var("HOME").unwrap_or_default();
            PathBuf::from(base).join(".local/share/solana/install/active_release/bin")
        },
        PathBuf::from,
    );
    home.join(name)
}

pub fn solana() -> PathBuf {
    tool("SOLANA_BIN", "solana")
}
pub fn keygen() -> PathBuf {
    tool("SOLANA_KEYGEN", "solana-keygen")
}
pub fn build_sbf() -> PathBuf {
    tool("CARGO_BUILD_SBF", "cargo-build-sbf")
}
pub fn test_validator() -> PathBuf {
    tool("SOLANA_TEST_VALIDATOR", "solana-test-validator")
}

/// Ephemeral signers, unlinked on drop.
pub struct Keys {
    dir: PathBuf,
    public: Vec<(String, String)>,
}

impl Keys {
    /// Mint one fresh key per roster role.
    pub fn mint(parent: &Path) -> Result<Self> {
        let dir = parent.join("keys");
        fs::create_dir_all(&dir)?;
        let mut public = Vec::new();
        for name in KEY_NAMES {
            let path = dir.join(format!("{name}.json"));
            let created = Command::new(keygen())
                .args(["new", "--no-bip39-passphrase", "--silent", "--force", "-o"])
                .arg(&path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            if !created.success() {
                return Err(format!("solana-keygen could not create {name}").into());
            }
            let shown = Command::new(keygen()).arg("pubkey").arg(&path).output()?;
            if !shown.status.success() {
                return Err(format!("solana-keygen could not read {name}").into());
            }
            public.push((
                name.to_string(),
                String::from_utf8(shown.stdout)?.trim().to_string(),
            ));
        }
        Ok(Self { dir, public })
    }

    pub fn public_key(&self, name: &str) -> Option<&str> {
        self.public
            .iter()
            .find(|(role, _)| role == name)
            .map(|(_, key)| key.as_str())
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        KEY_NAMES
            .iter()
            .map(|name| self.dir.join(format!("{name}.json")))
            .collect()
    }

    /// The roster as the browser is shown it: role and public key, never a
    /// secret and never a file path.
    pub fn roster(&self) -> Value {
        Value::Array(
            self.public
                .iter()
                .map(|(role, key)| json!({"role": role, "pubkey": key}))
                .collect(),
        )
    }

    /// Export the public keys the plan emitter reads.
    pub fn export(&self) {
        for (name, variable) in KEY_VARIABLES {
            if let Some(key) = self.public_key(name) {
                env::set_var(variable, key);
            }
        }
    }
}

impl Drop for Keys {
    fn drop(&mut self) {
        for path in self.paths() {
            let _ignored = fs::remove_file(path);
        }
        let _ignored = fs::remove_dir(&self.dir);
    }
}

/// The built, hashed, explicitly non-production program artifact.
pub struct Artifact {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_profile: &'static str,
}

/// Build the mock-source ELF and hash the bytes that were actually written.
///
/// The digest the bench displays is computed here, from the file the
/// validator is about to load — not read from a manifest, so a stale seal
/// cannot make the banner say something the running bank does not.
pub fn build_artifact(program_manifest: &Path, out_dir: &Path, log: &Path) -> Result<Artifact> {
    let output = Command::new(build_sbf())
        .arg("--manifest-path")
        .arg(program_manifest)
        .arg("--sbf-out-dir")
        .arg(out_dir)
        .args(["--features", "non-production-mock-source"])
        .env("CARGO_NET_OFFLINE", "true")
        .output()?;
    fs::write(log, [output.stdout.clone(), output.stderr.clone()].concat())?;
    if !output.status.success() {
        return Err(format!(
            "cargo-build-sbf failed; see {}",
            log.display()
        )
        .into());
    }
    let path = out_dir.join("clutch_sbf.so");
    let image = fs::read(&path)?;
    Ok(Artifact {
        bytes: image.len() as u64,
        sha256: clutch_sbf_harness::hex_encode(
            solana_sha256_hasher::hash(&image).to_bytes().as_slice(),
        ),
        path,
        source_profile: "NON-PRODUCTION-non-production-mock-source",
    })
}

/// A running local validator, killed on drop.
pub struct Validator {
    child: Child,
    pub url: String,
    pub ledger: PathBuf,
}

impl Validator {
    /// Start a fresh ledger with the ELF loaded and every genesis row installed.
    pub fn start(
        work: &Path,
        plan_dir: &Path,
        artifact: &Artifact,
        program_id: &str,
        payer: &str,
        rpc_port: u16,
        faucet_port: u16,
    ) -> Result<Self> {
        let ledger = work.join("ledger");
        let mut command = Command::new(test_validator());
        command
            .arg("--ledger")
            .arg(&ledger)
            .args(["--reset", "--quiet", "--rpc-port"])
            .arg(rpc_port.to_string())
            .arg("--faucet-port")
            .arg(faucet_port.to_string())
            .arg("--mint")
            .arg(payer)
            .arg("--bpf-program")
            .arg(program_id)
            .arg(&artifact.path);
        for line in fs::read_to_string(plan_dir.join("genesis.txt"))?.lines() {
            let mut fields = line.split_whitespace();
            let (Some(_role), Some(address), Some(file)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            command.arg("--account").arg(address).arg(plan_dir.join(file));
        }
        let log = fs::File::create(work.join("validator.log"))?;
        let child = command
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        Ok(Self {
            child,
            url: format!("http://127.0.0.1:{rpc_port}"),
            ledger,
        })
    }

    /// Wait for slot one **and** an executable program account, exactly the
    /// readiness the committed script requires before it submits anything.
    ///
    /// The liveness check on our own child is load-bearing, not defensive
    /// tidiness: if this validator could not bind its port because *another*
    /// bank is already there, the child exits and the two probes below start
    /// answering for a stranger's ledger.  A walk driven against someone
    /// else's bank produces confident, meaningless evidence, so a dead child
    /// is a hard failure rather than a reason to keep polling.
    pub fn await_ready(&mut self, program_id: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait()? {
                return Err(format!(
                    "the local validator exited before it was ready ({status}); \
                     something else may already hold {}",
                    self.url
                )
                .into());
            }
            let slot = crate::rpc::current_slot(&self.url).unwrap_or(0);
            let executable = crate::rpc::rpc(
                &self.url,
                "getAccountInfo",
                &json!([program_id, {"encoding": "base64"}]),
            )
            .ok()
            .and_then(|result| {
                result
                    .get("value")
                    .and_then(|value| value.get("executable"))
                    .and_then(Value::as_bool)
            })
            .unwrap_or(false);
            if slot >= 1 && executable {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(250));
        }
        Err("local validator never exposed the executable program after slot zero".into())
    }
}

impl Drop for Validator {
    fn drop(&mut self) {
        let _ignored = self.child.kill();
        let _ignored = self.child.wait();
    }
}

/// Refuse to run if something is already serving the chosen RPC port.
pub fn refuse_occupied_port(url: &str) -> Result<()> {
    if crate::rpc::current_slot(url).is_ok() {
        return Err(format!("{url} was already serving before the bench started").into());
    }
    Ok(())
}
