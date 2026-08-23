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
    let cache_root = env::var("CLUTCH_AGAVE_LOOPBACK_CACHE").map_or_else(
        |_| crate::repo_path(".cache/agave-loopback-validator"),
        PathBuf::from,
    );
    env::var("CLUTCH_LOOPBACK_TEST_VALIDATOR")
        .or_else(|_| env::var("SOLANA_TEST_VALIDATOR"))
        .map_or_else(
            |_| cache_root.join("bin/solana-test-validator"),
            PathBuf::from,
        )
}

fn verify_validator_runtime(binary: &Path, evidence: &Path) -> Result<()> {
    let verifier = crate::repo_path("tools/agave-loopback-validator/verify-runtime.py");
    let output = Command::new("python3")
        .arg(&verifier)
        .arg("--binary")
        .arg(binary)
        .output()?;
    let mut transcript = output.stdout;
    transcript.extend_from_slice(&output.stderr);
    fs::write(evidence, &transcript)?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&transcript);
    Err(format!(
        "refusing local validator {}: {}; stock Agave 4.0.2 has wildcard-listener risk",
        binary.display(),
        detail.trim()
    )
    .into())
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
        return Err(format!("cargo-build-sbf failed; see {}", log.display()).into());
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
    binary: PathBuf,
    rpc_port: u16,
    faucet_port: u16,
    pub url: String,
    pub ledger: PathBuf,
}

/// Every listener assigned to one loopback validator instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatorNetwork<'a> {
    pub rpc_port: u16,
    pub faucet_port: u16,
    pub gossip_port: u16,
    pub dynamic_port_range: &'a str,
}

impl Validator {
    /// Start a fresh ledger with the ELF loaded and every genesis row installed.
    pub fn start(
        work: &Path,
        plan_dir: &Path,
        artifact: &Artifact,
        program_id: &str,
        payer: &str,
        network: ValidatorNetwork<'_>,
    ) -> Result<Self> {
        let ledger = work.join("ledger");
        let binary = test_validator();
        verify_validator_runtime(&binary, &work.join("validator-runtime.txt"))?;
        let mut command = Command::new(&binary);
        command
            .arg("--ledger")
            .arg(&ledger)
            .args(["--reset", "--quiet"]);
        append_validator_network_args(
            &mut command,
            network.rpc_port,
            network.faucet_port,
            network.gossip_port,
            network.dynamic_port_range,
        )?;
        command
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
            command
                .arg("--account")
                .arg(address)
                .arg(plan_dir.join(file));
        }
        let log = fs::File::create(work.join("validator.log"))?;
        let child = command
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        Ok(Self {
            child,
            binary,
            rpc_port: network.rpc_port,
            faucet_port: network.faucet_port,
            url: format!("http://127.0.0.1:{}", network.rpc_port),
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

    /// Record the exact-PID listener proof before and after protocol traffic.
    pub fn probe_listeners(&mut self, evidence: &Path) -> Result<()> {
        if let Some(status) = self.child.try_wait()? {
            return Err(
                format!("the local validator exited before its listener probe ({status})").into(),
            );
        }
        let probe = crate::repo_path("tools/agave-loopback-validator/probe-listeners.sh");
        let output = Command::new(&probe)
            .arg(self.child.id().to_string())
            .arg(self.rpc_port.to_string())
            .arg(self.faucet_port.to_string())
            .arg(&self.binary)
            .output()?;
        let mut transcript = output.stdout;
        transcript.extend_from_slice(&output.stderr);
        fs::write(evidence, &transcript)?;
        if !output.status.success() {
            return Err(format!(
                "validator listener isolation probe failed; see {}",
                evidence.display()
            )
            .into());
        }
        Ok(())
    }
}

impl Drop for Validator {
    fn drop(&mut self) {
        let _ignored = self.child.kill();
        let _ignored = self.child.wait();
    }
}

/// Validate the complete explicit listener plan before starting a validator.
///
/// `solana-test-validator` otherwise chooses a broad implicit dynamic range,
/// which makes independently configured local lanes steal one another's ports.
/// Agave also reserves `rpc_port + 1` for RPC WebSocket without exposing a
/// separate CLI flag, so that derived listener participates in every overlap
/// check. The required repository-patched binary covers every known bind path;
/// `--bind-address` remains explicit and exact-PID probes independently refuse
/// wildcard RPC, WebSocket, faucet, QUIC, or UDP sockets at runtime.
pub fn validate_validator_network(
    http_port: Option<u16>,
    rpc_port: u16,
    faucet_port: u16,
    gossip_port: u16,
    dynamic_port_range: &str,
) -> Result<()> {
    let rpc_websocket_port = rpc_port
        .checked_add(1)
        .ok_or("rpc port 65535 leaves no port for RPC WebSocket")?;
    let (dynamic_start, dynamic_end) = dynamic_port_range.split_once('-').ok_or_else(|| {
        format!("invalid dynamic port range {dynamic_port_range:?}; expected START-END")
    })?;
    let dynamic_start: u16 = dynamic_start.parse()?;
    let dynamic_end: u16 = dynamic_end.parse()?;
    if dynamic_start == 0 || dynamic_start > dynamic_end {
        return Err(format!(
            "invalid dynamic port range {dynamic_port_range:?}; expected nonzero START <= END"
        )
        .into());
    }

    let mut fixed = vec![
        ("rpc", rpc_port),
        ("rpc websocket", rpc_websocket_port),
        ("faucet", faucet_port),
        ("gossip", gossip_port),
    ];
    if let Some(port) = http_port {
        fixed.push(("http", port));
    }
    for (name, port) in &fixed {
        if *port == 0 {
            return Err(format!("{name} port must be nonzero").into());
        }
        if (dynamic_start..=dynamic_end).contains(port) {
            return Err(format!(
                "{name} port {port} overlaps dynamic port range {dynamic_port_range}"
            )
            .into());
        }
    }
    for left in 0..fixed.len() {
        for right in left + 1..fixed.len() {
            if fixed[left].1 == fixed[right].1 {
                return Err(format!(
                    "{} and {} ports both use {}",
                    fixed[left].0, fixed[right].0, fixed[left].1
                )
                .into());
            }
        }
    }
    Ok(())
}

fn append_validator_network_args(
    command: &mut Command,
    rpc_port: u16,
    faucet_port: u16,
    gossip_port: u16,
    dynamic_port_range: &str,
) -> Result<()> {
    validate_validator_network(None, rpc_port, faucet_port, gossip_port, dynamic_port_range)?;
    command
        // Still required for the patched validator's gossip/node socket paths.
        .args(["--bind-address", "127.0.0.1"])
        .arg("--rpc-port")
        .arg(rpc_port.to_string())
        .arg("--faucet-port")
        .arg(faucet_port.to_string())
        .arg("--gossip-port")
        .arg(gossip_port.to_string())
        .arg("--dynamic-port-range")
        .arg(dynamic_port_range);
    Ok(())
}

/// Refuse to run if something is already serving the chosen RPC port.
pub fn refuse_occupied_port(url: &str) -> Result<()> {
    if crate::rpc::current_slot(url).is_ok() {
        return Err(format!("{url} was already serving before the bench started").into());
    }
    Ok(())
}

#[cfg(test)]
mod network_tests {
    use super::{append_validator_network_args, validate_validator_network};
    use std::process::Command;

    #[test]
    fn accepts_disjoint_explicit_ports() {
        validate_validator_network(Some(9130), 9137, 9139, 9200, "9201-9250").unwrap();
    }

    #[test]
    fn refuses_malformed_reversed_and_overlapping_ranges() {
        assert!(validate_validator_network(None, 9137, 9139, 9200, "9201").is_err());
        assert!(validate_validator_network(None, 9137, 9139, 9200, "9250-9201").is_err());
        assert!(validate_validator_network(None, 9137, 9139, 9200, "9199-9201").is_err());
        assert!(validate_validator_network(Some(9201), 9137, 9139, 9200, "9201-9250").is_err());
    }

    #[test]
    fn refuses_duplicate_fixed_ports() {
        assert!(validate_validator_network(Some(9130), 9137, 9137, 9200, "9201-9250").is_err());
    }

    #[test]
    fn reserves_the_implicit_rpc_websocket_port() {
        assert!(validate_validator_network(Some(9130), 9137, 9138, 9200, "9201-9250").is_err());
        assert!(validate_validator_network(Some(9130), 9137, 9139, 9138, "9201-9250").is_err());
        assert!(validate_validator_network(Some(9138), 9137, 9139, 9200, "9201-9250").is_err());
        assert!(validate_validator_network(Some(9130), 9137, 9139, 9200, "9138-9199").is_err());
        assert!(validate_validator_network(None, u16::MAX, 9139, 9200, "9201-9250").is_err());
    }

    #[test]
    fn emits_every_validator_network_flag_explicitly() {
        let mut command = Command::new("solana-test-validator");
        append_validator_network_args(&mut command, 9137, 9139, 9200, "9201-9250").unwrap();
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--bind-address",
                "127.0.0.1",
                "--rpc-port",
                "9137",
                "--faucet-port",
                "9139",
                "--gossip-port",
                "9200",
                "--dynamic-port-range",
                "9201-9250",
            ]
        );
    }
}
