//! Explicit local-validator session and ephemeral-key lifecycle.
//!
//! This module is intentionally incapable of discovering a wallet. It accepts
//! one absolute session root, creates fresh key material only below that root,
//! and exposes public identities plus explicit paths for local validator
//! processes. It has no RPC client, transaction signer, or submit method.

use solana_address::Address;
use solana_keypair::{write_keypair_file, Keypair, Signer};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions, Permissions};
use std::io::Write as _;
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, SessionError>;

/// Marker proving a directory was created by this lifecycle owner.
pub const SESSION_MARKER: &str = "dragons-clutch/local-validator-session/v1\n";
/// Public, secret-free configuration artifact written into every session.
pub const PUBLIC_MANIFEST_SCHEMA: &str = "dragons-clutch/local-validator-public-manifest/v1";

#[derive(Debug)]
pub enum SessionError {
    InvalidRoot(&'static str),
    InvalidEndpoint(&'static str),
    InvalidSource(&'static str),
    InvalidRelease(&'static str),
    DuplicateKeyRole,
    ExistingPath,
    MarkerMismatch,
    Io(std::io::Error),
    KeyWrite(String),
}

impl core::fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidRoot(detail)
            | Self::InvalidEndpoint(detail)
            | Self::InvalidSource(detail)
            | Self::InvalidRelease(detail) => formatter.write_str(detail),
            Self::DuplicateKeyRole => {
                formatter.write_str("ephemeral key roles contain a duplicate")
            }
            Self::ExistingPath => formatter.write_str("local session root already exists"),
            Self::MarkerMismatch => {
                formatter.write_str("local session ownership marker mismatches")
            }
            Self::Io(error) => write!(formatter, "local session I/O failed: {error}"),
            Self::KeyWrite(error) => write!(formatter, "ephemeral key write failed: {error}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<std::io::Error> for SessionError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Explicit loopback ports owned by one validator session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalValidatorPorts {
    pub rpc: u16,
    pub rpc_websocket: u16,
    pub faucet: u16,
    pub gossip: u16,
    pub dynamic_start: u16,
    pub dynamic_end: u16,
}

impl LocalValidatorPorts {
    pub fn validate(self) -> Result<()> {
        let fixed = [self.rpc, self.rpc_websocket, self.faucet, self.gossip];
        if fixed.iter().any(|port| *port == 0)
            || self.dynamic_start == 0
            || self.dynamic_start > self.dynamic_end
        {
            return Err(SessionError::InvalidEndpoint(
                "local validator ports are invalid",
            ));
        }
        let mut unique = BTreeSet::new();
        for port in fixed {
            if !unique.insert(port) || (self.dynamic_start..=self.dynamic_end).contains(&port) {
                return Err(SessionError::InvalidEndpoint(
                    "local validator fixed and dynamic ports overlap",
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn rpc_http(self) -> String {
        format!("http://127.0.0.1:{}", self.rpc)
    }

    #[must_use]
    pub fn rpc_websocket(self) -> String {
        format!("ws://127.0.0.1:{}", self.rpc_websocket)
    }
}

/// Where authenticated real source bytes may be acquired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealSourceAcquisitionV3 {
    /// Use a SHA-256-pinned capture already present on the local machine.
    PinnedLocalCapture { capture_manifest_sha256: [u8; 32] },
    /// Permit a separate bounded reader to perform finalized public RPC reads.
    /// This is data acquisition only; no remote write endpoint exists here.
    BoundedPublicRead {
        https_rpc_url: String,
        maximum_account_reads: u16,
    },
}

impl RealSourceAcquisitionV3 {
    fn validate(&self) -> Result<()> {
        match self {
            Self::PinnedLocalCapture {
                capture_manifest_sha256,
            } => require_digest(*capture_manifest_sha256, "source capture digest is zero"),
            Self::BoundedPublicRead {
                https_rpc_url,
                maximum_account_reads,
            } => {
                if !https_rpc_url.starts_with("https://")
                    || https_rpc_url.contains('@')
                    || *maximum_account_reads == 0
                    || *maximum_account_reads > 1_024
                {
                    return Err(SessionError::InvalidSource(
                        "public source acquisition must be bounded, credential-free HTTPS reads",
                    ));
                }
                Ok(())
            }
        }
    }

    fn public_description(&self) -> String {
        match self {
            Self::PinnedLocalCapture {
                capture_manifest_sha256,
            } => format!("pinned-local-capture:{}", hex(capture_manifest_sha256)),
            Self::BoundedPublicRead {
                https_rpc_url,
                maximum_account_reads,
            } => format!("bounded-public-read:{https_rpc_url}:max={maximum_account_reads}"),
        }
    }
}

/// Complete source binding required before a SourcePlane V3 plan is built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealSourceConfigV3 {
    pub provider_program: Address,
    pub provider_config: Address,
    pub feed_id: [u8; 32],
    pub provider_release_sha256: [u8; 32],
    pub source_adapter_program: Address,
    pub source_adapter_release_sha256: [u8; 32],
    pub source_spec_id: [u8; 32],
    pub acquisition: RealSourceAcquisitionV3,
}

impl RealSourceConfigV3 {
    pub fn validate(&self) -> Result<()> {
        if self.provider_program == Address::default()
            || self.provider_config == Address::default()
            || self.source_adapter_program == Address::default()
            || self.provider_program == self.source_adapter_program
        {
            return Err(SessionError::InvalidSource(
                "real source program and Config identities are invalid",
            ));
        }
        require_digest(self.feed_id, "real source feed identity is zero")?;
        require_digest(
            self.provider_release_sha256,
            "provider release digest is zero",
        )?;
        require_digest(
            self.source_adapter_release_sha256,
            "source adapter release digest is zero",
        )?;
        require_digest(self.source_spec_id, "source specification identity is zero")?;
        self.acquisition.validate()
    }
}

/// Exact program release loaded by the local validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalProgramRelease {
    pub program_id: Address,
    pub elf_sha256: [u8; 32],
}

impl LocalProgramRelease {
    pub fn validate(self) -> Result<()> {
        if self.program_id == Address::default() {
            return Err(SessionError::InvalidRelease("program identity is zero"));
        }
        require_digest(self.elf_sha256, "program ELF digest is zero")
    }
}

/// No ambient CLI config or wallet path participates in this configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSessionConfig {
    pub root: PathBuf,
    pub ports: LocalValidatorPorts,
    pub clutch_release: LocalProgramRelease,
    pub source: RealSourceConfigV3,
}

impl LocalSessionConfig {
    pub fn validate(&self) -> Result<()> {
        validate_root(&self.root)?;
        self.ports.validate()?;
        self.clutch_release.validate()?;
        self.source.validate()
    }
}

/// Paths created and exclusively owned by one local session.
#[derive(Debug)]
pub struct SessionLayout {
    root: PathBuf,
    ledger: PathBuf,
    ephemeral_keys: PathBuf,
    public_manifest: PathBuf,
    marker: PathBuf,
}

impl SessionLayout {
    /// Create one new session. Existing paths are refused, never reused.
    pub fn initialize(config: &LocalSessionConfig) -> Result<Self> {
        config.validate()?;
        if config.root.exists() {
            return Err(SessionError::ExistingPath);
        }
        fs::create_dir(&config.root)?;
        fs::set_permissions(&config.root, Permissions::from_mode(0o700))?;

        let marker = config.root.join("SESSION_OWNER");
        let mut marker_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)?;
        marker_file.write_all(SESSION_MARKER.as_bytes())?;
        marker_file.sync_all()?;

        let ledger = config.root.join("ledger");
        let ephemeral_keys = config.root.join("ephemeral-keys");
        fs::create_dir(&ledger)?;
        fs::create_dir(&ephemeral_keys)?;
        fs::set_permissions(&ephemeral_keys, Permissions::from_mode(0o700))?;
        let public_manifest = config.root.join("public-session.txt");
        let layout = Self {
            root: config.root.clone(),
            ledger,
            ephemeral_keys,
            public_manifest,
            marker,
        };
        layout.write_public_manifest(config)?;
        Ok(layout)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn ledger(&self) -> &Path {
        &self.ledger
    }

    #[must_use]
    pub fn public_manifest(&self) -> &Path {
        &self.public_manifest
    }

    fn key_path(&self, role: EphemeralKeyRole) -> PathBuf {
        self.ephemeral_keys.join(role.file_name())
    }

    fn write_public_manifest(&self, config: &LocalSessionConfig) -> Result<()> {
        let mut body = String::new();
        writeln!(body, "schema={PUBLIC_MANIFEST_SCHEMA}").expect("String write is infallible");
        writeln!(body, "network=local-validator").expect("String write is infallible");
        writeln!(body, "rpc_http={}", config.ports.rpc_http()).expect("String write is infallible");
        writeln!(body, "rpc_websocket={}", config.ports.rpc_websocket())
            .expect("String write is infallible");
        writeln!(body, "faucet_port={}", config.ports.faucet).expect("String write is infallible");
        writeln!(body, "gossip_port={}", config.ports.gossip).expect("String write is infallible");
        writeln!(
            body,
            "dynamic_port_range={}-{}",
            config.ports.dynamic_start, config.ports.dynamic_end
        )
        .expect("String write is infallible");
        writeln!(body, "clutch_program={}", config.clutch_release.program_id)
            .expect("String write is infallible");
        writeln!(
            body,
            "clutch_elf_sha256={}",
            hex(&config.clutch_release.elf_sha256)
        )
        .expect("String write is infallible");
        writeln!(body, "provider_program={}", config.source.provider_program)
            .expect("String write is infallible");
        writeln!(body, "provider_config={}", config.source.provider_config)
            .expect("String write is infallible");
        writeln!(
            body,
            "provider_release_sha256={}",
            hex(&config.source.provider_release_sha256)
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "source_adapter_program={}",
            config.source.source_adapter_program
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "source_adapter_release_sha256={}",
            hex(&config.source.source_adapter_release_sha256)
        )
        .expect("String write is infallible");
        writeln!(body, "feed_id={}", hex(&config.source.feed_id))
            .expect("String write is infallible");
        writeln!(
            body,
            "source_spec_id={}",
            hex(&config.source.source_spec_id)
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "source_acquisition={}",
            config.source.acquisition.public_description()
        )
        .expect("String write is infallible");
        writeln!(body, "signing=not-exposed").expect("String write is infallible");
        writeln!(body, "submission=not-exposed").expect("String write is infallible");
        fs::write(&self.public_manifest, body)?;
        Ok(())
    }

    /// Remove only this marked session root. Callers must opt in explicitly.
    pub fn destroy(self) -> Result<()> {
        let marker = fs::read_to_string(&self.marker)?;
        if marker != SESSION_MARKER {
            return Err(SessionError::MarkerMismatch);
        }
        fs::remove_dir_all(&self.root)?;
        Ok(())
    }
}

/// Fixed local roles; none are a user's default Solana CLI wallet.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EphemeralKeyRole {
    ValidatorIdentity,
    VoteAuthority,
    Faucet,
    Payer,
    SecondOwner,
    SourceSubmitter,
    Keeper,
}

impl EphemeralKeyRole {
    const fn file_name(self) -> &'static str {
        match self {
            Self::ValidatorIdentity => "validator-identity.json",
            Self::VoteAuthority => "vote-authority.json",
            Self::Faucet => "faucet.json",
            Self::Payer => "payer.json",
            Self::SecondOwner => "second-owner.json",
            Self::SourceSubmitter => "source-submitter.json",
            Self::Keeper => "keeper.json",
        }
    }
}

/// Public view of one fresh, session-scoped key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicSessionKey {
    pub role: EphemeralKeyRole,
    pub address: Address,
    pub explicit_path: PathBuf,
}

struct OwnedSessionKey {
    public: PublicSessionKey,
    _secret: Keypair,
}

/// In-memory ownership plus public paths for freshly generated local keys.
///
/// There is deliberately no loader and no signer method. The only key files
/// represented here are ones created by this roster below the marked session.
pub struct EphemeralKeyRoster {
    keys: Vec<OwnedSessionKey>,
}

impl EphemeralKeyRoster {
    pub fn create(layout: &SessionLayout, roles: &[EphemeralKeyRole]) -> Result<Self> {
        if roles.is_empty() {
            return Err(SessionError::InvalidRoot("ephemeral key roster is empty"));
        }
        let mut unique = BTreeSet::new();
        let mut keys = Vec::with_capacity(roles.len());
        for role in roles {
            if !unique.insert(*role) {
                return Err(SessionError::DuplicateKeyRole);
            }
            let keypair = Keypair::new();
            let explicit_path = layout.key_path(*role);
            write_keypair_file(&keypair, &explicit_path)
                .map_err(|error| SessionError::KeyWrite(error.to_string()))?;
            fs::set_permissions(&explicit_path, Permissions::from_mode(0o600))?;
            let public = PublicSessionKey {
                role: *role,
                address: keypair.pubkey(),
                explicit_path,
            };
            keys.push(OwnedSessionKey {
                public,
                _secret: keypair,
            });
        }
        Ok(Self { keys })
    }

    #[must_use]
    pub fn public_keys(&self) -> Vec<PublicSessionKey> {
        self.keys.iter().map(|key| key.public.clone()).collect()
    }

    #[must_use]
    pub fn public_key(&self, role: EphemeralKeyRole) -> Option<&PublicSessionKey> {
        self.keys
            .iter()
            .find(|key| key.public.role == role)
            .map(|key| &key.public)
    }
}

fn validate_root(root: &Path) -> Result<()> {
    if !root.is_absolute() || root == Path::new("/") || root.ancestors().count() < 3 {
        return Err(SessionError::InvalidRoot(
            "local session root must be a narrow absolute path",
        ));
    }
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(SessionError::InvalidRoot(
            "local session root has no UTF-8 leaf",
        ))?;
    if name.is_empty() || name == ".solana" || name == "id.json" {
        return Err(SessionError::InvalidRoot(
            "local session root resembles a wallet path",
        ));
    }
    Ok(())
}

fn require_digest(value: [u8; 32], message: &'static str) -> Result<()> {
    if value == [0; 32] {
        Err(SessionError::InvalidSource(message))
    } else {
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("String write is infallible");
    }
    output
}

#[allow(dead_code)]
fn require_loopback_endpoint(url: &str, scheme: &str) -> Result<SocketAddr> {
    let authority = url
        .strip_prefix(scheme)
        .ok_or(SessionError::InvalidEndpoint(
            "loopback endpoint scheme mismatches",
        ))?;
    let socket: SocketAddr = authority
        .parse()
        .map_err(|_| SessionError::InvalidEndpoint("loopback endpoint is not a socket"))?;
    if !socket.ip().is_loopback() {
        return Err(SessionError::InvalidEndpoint(
            "local validator endpoint is not loopback",
        ));
    }
    Ok(socket)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_ports_are_disjoint_and_loopback() {
        let ports = LocalValidatorPorts {
            rpc: 9137,
            rpc_websocket: 9138,
            faucet: 9139,
            gossip: 9200,
            dynamic_start: 9201,
            dynamic_end: 9250,
        };
        assert!(ports.validate().is_ok());
        assert!(require_loopback_endpoint(&ports.rpc_http(), "http://").is_ok());
        assert!(require_loopback_endpoint(&ports.rpc_websocket(), "ws://").is_ok());
    }

    #[test]
    fn public_reads_are_explicitly_bounded_and_credential_free() {
        assert!(RealSourceAcquisitionV3::BoundedPublicRead {
            https_rpc_url: "https://api.devnet.solana.com".into(),
            maximum_account_reads: 16,
        }
        .validate()
        .is_ok());
        assert!(RealSourceAcquisitionV3::BoundedPublicRead {
            https_rpc_url: "https://token@example.invalid".into(),
            maximum_account_reads: 16,
        }
        .validate()
        .is_err());
    }
}
