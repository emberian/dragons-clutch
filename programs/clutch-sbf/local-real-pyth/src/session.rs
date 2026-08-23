//! Explicit local-validator session and ephemeral-key lifecycle.
//!
//! This module is intentionally incapable of discovering a wallet. It accepts
//! one absolute session root, creates fresh key material only below that root,
//! and exposes public identities plus explicit paths for local validator
//! processes. It has no RPC client, transaction signer, or submit method.

use solana_address::Address;
use solana_keypair::{write_keypair_file, Keypair, Signer};
use std::collections::BTreeSet;
use std::ffi::OsString;
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
pub const PUBLIC_MANIFEST_SCHEMA: &str = "dragons-clutch/local-validator-public-manifest/v6";

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
    /// Exact captured Pyth receiver that owns the already-posted feed.
    pub receiver_program: Address,
    pub receiver_program_data: Address,
    pub receiver_deployment_slot: u64,
    pub receiver_config: Address,
    pub receiver_release_sha256: [u8; 32],
    /// Exact first-party read-only parser selected by the Source release.
    pub parser_program: Address,
    pub parser_program_data: Address,
    pub parser_deployment_slot: u64,
    pub parser_config: Address,
    pub parser_release_sha256: [u8; 32],
    /// Exact physical `PriceUpdateV2` account consumed by the parser.
    pub feed_account: Address,
    pub feed_id: [u8; 32],
    /// Reviewed transport/router program used by the receiver release.
    pub transport_program: Address,
    pub transport_program_data: Address,
    pub transport_deployment_slot: u64,
    pub transport_release_sha256: [u8; 32],
    pub source_spec_id: [u8; 32],
    pub acquisition: RealSourceAcquisitionV3,
}

impl RealSourceConfigV3 {
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.receiver_program,
            self.receiver_program_data,
            self.receiver_config,
            self.parser_program,
            self.parser_program_data,
            self.parser_config,
            self.feed_account,
            self.transport_program,
            self.transport_program_data,
        ];
        if identities
            .iter()
            .any(|identity| *identity == Address::default())
            || identities.iter().enumerate().any(|(index, identity)| {
                identities[..index]
                    .iter()
                    .any(|previous| previous == identity)
            })
        {
            return Err(SessionError::InvalidSource(
                "real Source parser, receiver, transport, feed, and Config identities are invalid",
            ));
        }
        if self.receiver_deployment_slot == 0
            || self.parser_deployment_slot == 0
            || self.transport_deployment_slot == 0
        {
            return Err(SessionError::InvalidSource(
                "real Source deployment slots must be nonzero",
            ));
        }
        require_digest(self.feed_id, "real source feed identity is zero")?;
        require_digest(
            self.receiver_release_sha256,
            "receiver release digest is zero",
        )?;
        require_digest(self.parser_release_sha256, "parser release digest is zero")?;
        require_digest(
            self.transport_release_sha256,
            "transport release digest is zero",
        )?;
        require_digest(self.source_spec_id, "source specification identity is zero")?;
        self.acquisition.validate()
    }
}

/// Expected upgradeable program release loaded by the local validator.
/// Construction records Program, ProgramData, slot, and ELF digest; the
/// process launcher remains responsible for checking the ELF bytes before
/// using the argv.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalProgramRelease {
    pub program_id: Address,
    pub program_data: Address,
    pub deployment_slot: u64,
    pub elf_sha256: [u8; 32],
    pub elf_path: PathBuf,
}

/// Cross-manifest seal for a chain-attached local release. Semantic capability
/// rows stay owned by the capability-profile manifest; this record pins that
/// checked owner to the exact deployed ELF and local runtime coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedChainReleaseBinding {
    pub capability_manifest_sha256: [u8; 32],
    pub capability_profile_id: [u8; 32],
    pub source_commit: String,
    pub compiler_release_sha256: [u8; 32],
    pub source_neutral_sink: Address,
}

impl CheckedChainReleaseBinding {
    pub fn validate(&self) -> Result<()> {
        if self.capability_manifest_sha256 == [0; 32]
            || self.capability_profile_id == [0; 32]
            || self.compiler_release_sha256 == [0; 32]
            || self.source_neutral_sink == Address::default()
            || !matches!(self.source_commit.len(), 40 | 64)
            || !self
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(SessionError::InvalidRelease(
                "checked local chain-release binding is invalid",
            ));
        }
        Ok(())
    }
}

impl LocalProgramRelease {
    pub fn validate(&self) -> Result<()> {
        if self.program_id == Address::default()
            || self.program_data == Address::default()
            || self.program_id == self.program_data
            || self.deployment_slot == 0
            || !self.elf_path.is_absolute()
            || self.elf_path == Path::new("/")
            || self.elf_path.extension().and_then(|value| value.to_str()) != Some("so")
        {
            return Err(SessionError::InvalidRelease(
                "program identity or absolute ELF path is invalid",
            ));
        }
        if self.elf_sha256 == [0; 32] {
            return Err(SessionError::InvalidRelease("program ELF digest is zero"));
        }
        Ok(())
    }
}

/// No ambient CLI config or wallet path participates in this configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSessionConfig {
    pub root: PathBuf,
    pub ports: LocalValidatorPorts,
    pub clutch_release: LocalProgramRelease,
    pub checked_chain_release: CheckedChainReleaseBinding,
    /// External parser/transport releases loaded alongside Clutch. Source V3
    /// itself executes inside `clutch_release`, never as a second adapter ELF.
    pub external_program_releases: Vec<LocalProgramRelease>,
    pub source: RealSourceConfigV3,
}

impl LocalSessionConfig {
    pub fn validate(&self) -> Result<()> {
        validate_root(&self.root)?;
        self.ports.validate()?;
        self.clutch_release.validate()?;
        self.checked_chain_release.validate()?;
        self.source.validate()?;
        let clutch_identities = [
            self.clutch_release.program_id,
            self.clutch_release.program_data,
        ];
        let source_identities = [
            self.source.receiver_program,
            self.source.receiver_program_data,
            self.source.receiver_config,
            self.source.parser_program,
            self.source.parser_program_data,
            self.source.parser_config,
            self.source.feed_account,
            self.source.transport_program,
            self.source.transport_program_data,
        ];
        if source_identities
            .iter()
            .any(|identity| clutch_identities.contains(identity))
        {
            return Err(SessionError::InvalidRelease(
                "external Source infrastructure aliases the Clutch program",
            ));
        }
        let mut release_identities = BTreeSet::from([
            self.clutch_release.program_id,
            self.clutch_release.program_data,
        ]);
        let mut previous_program = None;
        for release in &self.external_program_releases {
            release.validate()?;
            if previous_program.is_some_and(|previous| previous >= release.program_id) {
                return Err(SessionError::InvalidRelease(
                    "local adapter releases are not in canonical program-ID order",
                ));
            }
            if !release_identities.insert(release.program_id) {
                return Err(SessionError::InvalidRelease(
                    "local program release identity is duplicated",
                ));
            }
            if !release_identities.insert(release.program_data) {
                return Err(SessionError::InvalidRelease(
                    "local ProgramData release identity is duplicated or aliased",
                ));
            }
            previous_program = Some(release.program_id);
        }
        let receiver_is_loaded = self.external_program_releases.iter().any(|release| {
            release.program_id == self.source.receiver_program
                && release.program_data == self.source.receiver_program_data
                && release.deployment_slot == self.source.receiver_deployment_slot
                && release.elf_sha256 == self.source.receiver_release_sha256
        });
        let parser_is_loaded = self.external_program_releases.iter().any(|release| {
            release.program_id == self.source.parser_program
                && release.program_data == self.source.parser_program_data
                && release.deployment_slot == self.source.parser_deployment_slot
                && release.elf_sha256 == self.source.parser_release_sha256
        });
        let transport_is_loaded = self.external_program_releases.iter().any(|release| {
            release.program_id == self.source.transport_program
                && release.program_data == self.source.transport_program_data
                && release.deployment_slot == self.source.transport_deployment_slot
                && release.elf_sha256 == self.source.transport_release_sha256
        });
        if !receiver_is_loaded || !parser_is_loaded || !transport_is_loaded {
            return Err(SessionError::InvalidRelease(
                "Source parser, receiver, and transport releases are not all loaded locally",
            ));
        }
        Ok(())
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
        writeln!(body, "release_coordinates=sealed").expect("String write is infallible");
        writeln!(
            body,
            "decoder_set={}",
            crate::account_index::CANONICAL_ACCOUNT_DECODER_SET
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "capability_manifest_sha256={}",
            hex(&config.checked_chain_release.capability_manifest_sha256)
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "capability_profile_identity={}",
            hex(&config.checked_chain_release.capability_profile_id)
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "source_commit={}",
            config.checked_chain_release.source_commit
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "compiler_release_sha256={}",
            hex(&config.checked_chain_release.compiler_release_sha256)
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "source_neutral_sink={}",
            config.checked_chain_release.source_neutral_sink
        )
        .expect("String write is infallible");
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
            "clutch_program_data={}",
            config.clutch_release.program_data
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "clutch_deployment_slot={}",
            config.clutch_release.deployment_slot
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "clutch_elf_sha256={}",
            hex(&config.clutch_release.elf_sha256)
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "clutch_elf_path={}",
            config.clutch_release.elf_path.display()
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "external_program_count={}",
            config.external_program_releases.len()
        )
        .expect("String write is infallible");
        for (index, release) in config.external_program_releases.iter().enumerate() {
            writeln!(
                body,
                "external_program_{index}_program={}",
                release.program_id
            )
            .expect("String write is infallible");
            writeln!(
                body,
                "external_program_{index}_program_data={}",
                release.program_data
            )
            .expect("String write is infallible");
            writeln!(
                body,
                "external_program_{index}_deployment_slot={}",
                release.deployment_slot
            )
            .expect("String write is infallible");
            writeln!(
                body,
                "external_program_{index}_elf_sha256={}",
                hex(&release.elf_sha256)
            )
            .expect("String write is infallible");
            writeln!(
                body,
                "external_program_{index}_elf_path={}",
                release.elf_path.display()
            )
            .expect("String write is infallible");
        }
        writeln!(body, "receiver_program={}", config.source.receiver_program)
            .expect("String write is infallible");
        writeln!(
            body,
            "receiver_program_data={}",
            config.source.receiver_program_data
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "receiver_deployment_slot={}",
            config.source.receiver_deployment_slot
        )
        .expect("String write is infallible");
        writeln!(body, "receiver_config={}", config.source.receiver_config)
            .expect("String write is infallible");
        writeln!(
            body,
            "receiver_release_sha256={}",
            hex(&config.source.receiver_release_sha256)
        )
        .expect("String write is infallible");
        writeln!(body, "parser_program={}", config.source.parser_program)
            .expect("String write is infallible");
        writeln!(
            body,
            "parser_program_data={}",
            config.source.parser_program_data
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "parser_deployment_slot={}",
            config.source.parser_deployment_slot
        )
        .expect("String write is infallible");
        writeln!(body, "parser_config={}", config.source.parser_config)
            .expect("String write is infallible");
        writeln!(
            body,
            "parser_release_sha256={}",
            hex(&config.source.parser_release_sha256)
        )
        .expect("String write is infallible");
        writeln!(body, "feed_account={}", config.source.feed_account)
            .expect("String write is infallible");
        writeln!(
            body,
            "transport_program={}",
            config.source.transport_program
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "transport_program_data={}",
            config.source.transport_program_data
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "transport_deployment_slot={}",
            config.source.transport_deployment_slot
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "transport_release_sha256={}",
            hex(&config.source.transport_release_sha256)
        )
        .expect("String write is infallible");
        writeln!(
            body,
            "source_series_program={}",
            config.clutch_release.program_id
        )
        .expect("String write is infallible");
        writeln!(body, "source_series_execution=inside-clutch-sbf")
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

/// One exact account body loaded at validator genesis. The role is descriptive;
/// the address and pinned body digest are authoritative construction facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGenesisAccountFile {
    pub role: String,
    pub address: Address,
    pub account_json: PathBuf,
    pub body_sha256: [u8; 32],
}

impl LocalGenesisAccountFile {
    fn validate(&self, session_root: &Path) -> Result<()> {
        if self.role.trim().is_empty()
            || self.address == Address::default()
            || self.body_sha256 == [0; 32]
            || !self.account_json.is_absolute()
            || self
                .account_json
                .starts_with(session_root.join("ephemeral-keys"))
        {
            return Err(SessionError::InvalidRelease(
                "local genesis account fixture is incomplete or key-like",
            ));
        }
        Ok(())
    }
}

/// Pure argv construction for the selected local validator. Building this
/// value does not inspect a process, open RPC, start a validator, or sign.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalValidatorInvocation {
    executable: PathBuf,
    arguments: Vec<OsString>,
}

impl LocalValidatorInvocation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        validator_binary: PathBuf,
        config: &LocalSessionConfig,
        layout: &SessionLayout,
        mint_authority: Address,
        warp_slot: u64,
        genesis_accounts: &[LocalGenesisAccountFile],
    ) -> Result<Self> {
        config.validate()?;
        if !validator_binary.is_absolute()
            || layout.root != config.root
            || mint_authority == Address::default()
            || warp_slot == 0
        {
            return Err(SessionError::InvalidRelease(
                "local validator invocation is not explicitly bound",
            ));
        }
        let mut addresses = BTreeSet::new();
        for account in genesis_accounts {
            account.validate(layout.root())?;
            if !addresses.insert(account.address) {
                return Err(SessionError::InvalidRelease(
                    "local genesis account address is duplicated",
                ));
            }
        }

        let mut arguments = vec![
            OsString::from("--ledger"),
            layout.ledger.as_os_str().to_owned(),
            OsString::from("--reset"),
            OsString::from("--quiet"),
            OsString::from("--bind-address"),
            OsString::from("127.0.0.1"),
            OsString::from("--rpc-port"),
            OsString::from(config.ports.rpc.to_string()),
            OsString::from("--faucet-port"),
            OsString::from(config.ports.faucet.to_string()),
            OsString::from("--gossip-port"),
            OsString::from(config.ports.gossip.to_string()),
            OsString::from("--dynamic-port-range"),
            OsString::from(format!(
                "{}-{}",
                config.ports.dynamic_start, config.ports.dynamic_end
            )),
            OsString::from("--mint"),
            OsString::from(mint_authority.to_string()),
            OsString::from("--warp-slot"),
            OsString::from(warp_slot.to_string()),
        ];
        for release in
            core::iter::once(&config.clutch_release).chain(&config.external_program_releases)
        {
            arguments.push(OsString::from("--bpf-program"));
            arguments.push(OsString::from(release.program_id.to_string()));
            arguments.push(release.elf_path.as_os_str().to_owned());
        }
        for account in genesis_accounts {
            arguments.push(OsString::from("--account"));
            arguments.push(OsString::from(account.address.to_string()));
            arguments.push(account.account_json.as_os_str().to_owned());
        }
        Ok(Self {
            executable: validator_binary,
            arguments,
        })
    }

    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
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

    #[test]
    fn source_executes_in_clutch_with_distinct_parser_receiver_and_transport() {
        let program = |byte, digest, name: &str| LocalProgramRelease {
            program_id: Address::new_from_array([byte; 32]),
            program_data: Address::new_from_array([byte + 20; 32]),
            deployment_slot: 100 + u64::from(byte),
            elf_sha256: [digest; 32],
            elf_path: PathBuf::from(format!("/tmp/{name}.so")),
        };
        let mut config = LocalSessionConfig {
            root: PathBuf::from("/tmp/dragons-clutch-session-test"),
            ports: LocalValidatorPorts {
                rpc: 9137,
                rpc_websocket: 9138,
                faucet: 9139,
                gossip: 9200,
                dynamic_start: 9201,
                dynamic_end: 9250,
            },
            clutch_release: program(1, 11, "clutch"),
            checked_chain_release: CheckedChainReleaseBinding {
                capability_manifest_sha256: [18; 32],
                capability_profile_id: [19; 32],
                source_commit: "20".repeat(20),
                compiler_release_sha256: [21; 32],
                source_neutral_sink: Address::new_from_array([22; 32]),
            },
            external_program_releases: vec![
                program(2, 12, "receiver"),
                program(3, 13, "parser"),
                program(4, 14, "transport"),
            ],
            source: RealSourceConfigV3 {
                receiver_program: Address::new_from_array([2; 32]),
                receiver_program_data: Address::new_from_array([22; 32]),
                receiver_deployment_slot: 102,
                receiver_config: Address::new_from_array([5; 32]),
                receiver_release_sha256: [12; 32],
                parser_program: Address::new_from_array([3; 32]),
                parser_program_data: Address::new_from_array([23; 32]),
                parser_deployment_slot: 103,
                parser_config: Address::new_from_array([6; 32]),
                parser_release_sha256: [13; 32],
                feed_account: Address::new_from_array([7; 32]),
                feed_id: [15; 32],
                transport_program: Address::new_from_array([4; 32]),
                transport_program_data: Address::new_from_array([24; 32]),
                transport_deployment_slot: 104,
                transport_release_sha256: [14; 32],
                source_spec_id: [16; 32],
                acquisition: RealSourceAcquisitionV3::PinnedLocalCapture {
                    capture_manifest_sha256: [17; 32],
                },
            },
        };
        assert!(config.validate().is_ok());
        config.source.parser_program = config.source.receiver_program;
        assert!(config.validate().is_err());
    }
}
