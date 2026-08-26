#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Offline construction and verification of checked dClutch SBF releases.
//!
//! A semantic release identity remains owned by its capability contract. This
//! crate binds that identity to exact build and Loader V3 evidence without
//! claiming to deploy, sign, or observe a network.

use std::{fmt, str};

use dclutch_core_contract::ContentId;
use dclutch_pyth_svm::{
    LoaderV3Error, ProgramDataV3View, ProgramV3View, PythReleaseV1, PythReleaseV1Error,
};
use sha2::{Digest, Sha256};

mod infrastructure;
mod multiprogram;
mod translation;

pub use infrastructure::*;
pub use multiprogram::*;
pub use translation::*;

/// Canonical checked-release magic.
pub const CHECKED_RELEASE_MAGIC_V1: [u8; 8] = *b"DCLTREL1";
/// Implemented checked-release schema.
pub const CHECKED_RELEASE_SCHEMA_V1: u16 = 1;
/// Fixed byte prefix before length-prefixed reproducibility text.
pub const CHECKED_RELEASE_FIXED_BYTES_V1: usize = 388;
/// Loader V3's fixed ProgramData metadata allocation width.
pub const LOADER_V3_PROGRAMDATA_METADATA_BYTES: usize = 45;
/// Exact Loader V3 Program account-data width.
pub const LOADER_V3_PROGRAM_BYTES: usize = 36;
/// Solana's maximum permitted data length for one account.
pub const SOLANA_MAX_PERMITTED_ACCOUNT_DATA_BYTES: usize = 10 * 1024 * 1024;
/// Largest ELF that fits in a Loader V3 ProgramData account without padding.
pub const LOADER_V3_MAX_ELF_BYTES: usize =
    SOLANA_MAX_PERMITTED_ACCOUNT_DATA_BYTES - LOADER_V3_PROGRAMDATA_METADATA_BYTES;
/// Canonical metadata-file header.
pub const RELEASE_METADATA_HEADER_V1: &str = "dclutch-release-metadata-v1";

const SEMANTIC_KIND_OFFSET: usize = 10;
const LOADER_KIND_OFFSET: usize = 11;
const AUTHORITY_KIND_OFFSET: usize = 12;
const ASSUMPTION_COUNT_OFFSET: usize = 13;
const RESERVED_OFFSET: usize = 14;
const MANIFEST_LENGTH_OFFSET: usize = 16;
const SEMANTIC_LENGTH_OFFSET: usize = 20;
const ELF_LENGTH_OFFSET: usize = 28;
const PROGRAM_LENGTH_OFFSET: usize = 36;
const PROGRAMDATA_LENGTH_OFFSET: usize = 44;
const DEPLOYMENT_SLOT_OFFSET: usize = 52;
const PROGRAMDATA_ELF_OFFSET_OFFSET: usize = 60;
const ARTIFACT_DIGEST_OFFSET: usize = 68;
const SEMANTIC_ID_OFFSET: usize = 100;
const PROGRAM_DIGEST_OFFSET: usize = 132;
const PROGRAMDATA_DIGEST_OFFSET: usize = 164;
const PROGRAM_ID_OFFSET: usize = 196;
const PROGRAMDATA_ID_OFFSET: usize = 228;
const LOADER_ID_OFFSET: usize = 260;
const UPGRADE_AUTHORITY_OFFSET: usize = 292;
const SOURCE_DIGEST_OFFSET: usize = 324;
const CARGO_LOCK_DIGEST_OFFSET: usize = 356;

const LOADER_KIND_UPGRADEABLE_V3: u8 = 1;
const AUTHORITY_NONE: u8 = 0;
const AUTHORITY_SOME: u8 = 1;
const ELF_HEADER_BYTES: usize = 64;
const ELF_CLASS_64: u8 = 2;
const ELF_DATA_LITTLE_ENDIAN: u8 = 1;
const ELF_CURRENT_VERSION: u8 = 1;
const ELF_TYPE_SHARED_OBJECT: u16 = 3;
/// Legacy eBPF envelope still admitted by the Solana sBPF loader.
const ELF_MACHINE_BPF: u16 = 247;
/// Registered Solana Binary Format envelope emitted by current platform-tools.
const ELF_MACHINE_SBF: u16 = 263;

/// Refusal from canonical metadata, evidence, manifest, or verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed or bounded input had the wrong byte length.
    InvalidLength,
    /// Checked-release magic was not canonical.
    InvalidMagic,
    /// A checked-release schema was not implemented.
    UnsupportedSchema,
    /// Reserved bytes were not zero.
    NonCanonicalReservedBytes,
    /// A semantic-preimage kind byte or label was unknown.
    UnknownSemanticKind,
    /// The persisted loader kind was not Loader V3.
    UnknownLoaderKind,
    /// A required byte input was empty.
    EmptyInput,
    /// A required identifier or digest was the all-zero sentinel.
    ZeroIdentifier,
    /// Program, ProgramData, and loader identifiers were not distinct.
    AliasedLoaderIdentity,
    /// Reproducibility metadata was missing, reordered, duplicated, or trailing.
    InvalidMetadata,
    /// A metadata boolean was not the one canonical spelling.
    InvalidBoolean,
    /// A hexadecimal field was not exactly 64 lowercase hexadecimal digits.
    InvalidHex,
    /// A text field was empty, multiline, non-ASCII, or too wide for V1.
    InvalidText,
    /// Assumptions were absent, duplicated, or not strictly sorted.
    NonCanonicalAssumptions,
    /// A length calculation or integer conversion overflowed.
    ArithmeticOverflow,
    /// The supplied artifact was not a supported SBF ELF envelope.
    InvalidSbfElf,
    /// The ELF plus mandatory Loader V3 metadata cannot fit in one account.
    ArtifactExceedsLoaderLimit,
    /// The existing Loader V3 parser refused account data.
    LoaderV3(LoaderV3Error),
    /// Program account data named a different ProgramData account.
    ProgramDataLinkMismatch,
    /// Account owner/executable observations did not describe Loader V3.
    LoaderObservationMismatch,
    /// ProgramData was too short to contain the fixed metadata and exact ELF.
    ProgramDataTooShort,
    /// ProgramData exceeded Solana's maximum permitted account-data length.
    ProgramDataExceedsAccountLimit,
    /// The exact ELF did not occur at the fixed ProgramData payload offset.
    DeployedElfMismatch,
    /// Bytes after the checked ELF contained nonzero data.
    NonZeroProgramDataPadding,
    /// A Pyth semantic preimage was rejected by its existing semantic owner.
    InvalidPythRelease(PythReleaseV1Error),
    /// Optional upgrade-authority bytes were noncanonical.
    NonCanonicalUpgradeAuthority,
    /// The binary manifest's stated length did not equal its exact bytes.
    InvalidManifestLength,
    /// Supplied evidence rebuilt to a different checked manifest.
    CheckedManifestMismatch,
    /// A checked release could not be projected into the canonical onchain
    /// artifact-release record.
    InvalidArtifactRelease,
    /// A five-role execution release set was malformed or did not bind the
    /// supplied checked artifacts exactly.
    InvalidExecutionReleaseSet,
    /// A multiprogram release evidence manifest was malformed or noncanonical.
    InvalidMultiprogramManifest,
    /// Supplied checked-release manifests rebuilt to a different multiprogram
    /// evidence manifest.
    CheckedMultiprogramManifestMismatch,
    /// A checked infrastructure evidence manifest was malformed or noncanonical.
    InvalidInfrastructureManifest,
    /// Supplied checked manifests rebuilt to different infrastructure evidence.
    CheckedInfrastructureManifestMismatch,
    /// Core, Registry, and Rent infrastructure must all be immutable.
    InfrastructureMustBeImmutable,
    /// A translation-validation evidence input or manifest was malformed.
    InvalidTranslationValidation,
    /// Supplied translation-validation inputs rebuilt to another manifest.
    CheckedTranslationValidationMismatch,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Error {}

impl From<LoaderV3Error> for Error {
    fn from(value: LoaderV3Error) -> Self {
        Self::LoaderV3(value)
    }
}

/// Result alias for checked-release operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Existing semantic owner whose exact preimage is being bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SemanticPreimageKindV1 {
    /// Capability-owned canonical preimage; the capability remains its decoder.
    Capability = 0,
    /// Exact canonical [`PythReleaseV1`] preimage, decoded by `dclutch-pyth-svm`.
    PythReleaseV1 = 1,
}

impl SemanticPreimageKindV1 {
    /// Parse the canonical metadata label.
    pub fn parse(label: &str) -> Result<Self> {
        match label {
            "capability" => Ok(Self::Capability),
            "pyth-v1" => Ok(Self::PythReleaseV1),
            _ => Err(Error::UnknownSemanticKind),
        }
    }

    /// Return the canonical metadata and text-projection label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Capability => "capability",
            Self::PythReleaseV1 => "pyth-v1",
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Capability),
            1 => Ok(Self::PythReleaseV1),
            _ => Err(Error::UnknownSemanticKind),
        }
    }
}

/// Canonical reproducibility metadata supplied independently of artifact bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildMetadataV1 {
    semantic_kind: SemanticPreimageKindV1,
    program_id: [u8; 32],
    programdata_id: [u8; 32],
    loader_program_id: [u8; 32],
    program_owner: [u8; 32],
    program_executable: bool,
    programdata_owner: [u8; 32],
    programdata_executable: bool,
    source_digest: [u8; 32],
    cargo_lock_digest: [u8; 32],
    source_revision: String,
    rustc_version: String,
    solana_version: String,
    cargo_build_sbf_version: String,
    target_triple: String,
    build_command: String,
    assumptions: Vec<String>,
}

impl BuildMetadataV1 {
    /// Parse an exact canonical Metadata V1 text file.
    pub fn parse(text: &str) -> Result<Self> {
        if !text.ends_with('\n') || text.as_bytes().contains(&b'\r') {
            return Err(Error::InvalidMetadata);
        }
        let mut lines = text.split_terminator('\n');
        if lines.next() != Some(RELEASE_METADATA_HEADER_V1) {
            return Err(Error::InvalidMetadata);
        }
        let semantic_kind = SemanticPreimageKindV1::parse(value(&mut lines, "semantic_kind=")?)?;
        let program_id = decode_hex_32(value(&mut lines, "program_id=")?)?;
        let programdata_id = decode_hex_32(value(&mut lines, "programdata_id=")?)?;
        let loader_program_id = decode_hex_32(value(&mut lines, "loader_program_id=")?)?;
        let program_owner = decode_hex_32(value(&mut lines, "program_owner=")?)?;
        let program_executable = parse_bool(value(&mut lines, "program_executable=")?)?;
        let programdata_owner = decode_hex_32(value(&mut lines, "programdata_owner=")?)?;
        let programdata_executable = parse_bool(value(&mut lines, "programdata_executable=")?)?;
        let source_digest = decode_hex_32(value(&mut lines, "source_digest=")?)?;
        let cargo_lock_digest = decode_hex_32(value(&mut lines, "cargo_lock_digest=")?)?;
        let source_revision = value(&mut lines, "source_revision=")?.to_owned();
        let rustc_version = value(&mut lines, "rustc_version=")?.to_owned();
        let solana_version = value(&mut lines, "solana_version=")?.to_owned();
        let cargo_build_sbf_version = value(&mut lines, "cargo_build_sbf_version=")?.to_owned();
        let target_triple = value(&mut lines, "target_triple=")?.to_owned();
        let build_command = value(&mut lines, "build_command=")?.to_owned();
        let mut assumptions = Vec::new();
        for line in lines {
            let assumption = line
                .strip_prefix("assumption=")
                .ok_or(Error::InvalidMetadata)?;
            assumptions.push(assumption.to_owned());
        }
        let result = Self {
            semantic_kind,
            program_id,
            programdata_id,
            loader_program_id,
            program_owner,
            program_executable,
            programdata_owner,
            programdata_executable,
            source_digest,
            cargo_lock_digest,
            source_revision,
            rustc_version,
            solana_version,
            cargo_build_sbf_version,
            target_triple,
            build_command,
            assumptions,
        };
        result.validate()?;
        Ok(result)
    }

    /// Return the selected existing semantic-preimage profile.
    pub const fn semantic_kind(&self) -> SemanticPreimageKindV1 {
        self.semantic_kind
    }

    /// Return the observed executable Program account identity.
    pub const fn program_id(&self) -> [u8; 32] {
        self.program_id
    }

    /// Return the observed ProgramData account identity.
    pub const fn programdata_id(&self) -> [u8; 32] {
        self.programdata_id
    }

    /// Return the observed Loader V3 program identity.
    pub const fn loader_program_id(&self) -> [u8; 32] {
        self.loader_program_id
    }

    /// Return the exact committed assumptions in canonical order.
    pub fn assumptions(&self) -> &[String] {
        &self.assumptions
    }

    fn validate(&self) -> Result<()> {
        for identifier in [
            self.program_id,
            self.programdata_id,
            self.loader_program_id,
            self.source_digest,
            self.cargo_lock_digest,
        ] {
            require_nonzero(&identifier)?;
        }
        if self.program_id == self.programdata_id
            || self.program_id == self.loader_program_id
            || self.programdata_id == self.loader_program_id
        {
            return Err(Error::AliasedLoaderIdentity);
        }
        if self.program_owner != self.loader_program_id
            || self.programdata_owner != self.loader_program_id
            || !self.program_executable
            || self.programdata_executable
        {
            return Err(Error::LoaderObservationMismatch);
        }
        for text in [
            self.source_revision.as_str(),
            self.rustc_version.as_str(),
            self.solana_version.as_str(),
            self.cargo_build_sbf_version.as_str(),
            self.target_triple.as_str(),
            self.build_command.as_str(),
        ] {
            validate_text(text)?;
        }
        validate_assumptions(&self.assumptions)
    }
}

/// Exact byte evidence used to construct or verify one checked release.
#[derive(Clone, Copy, Debug)]
pub struct ReleaseEvidenceV1<'a> {
    /// Exact built SBF ELF file bytes.
    pub elf: &'a [u8],
    /// Exact capability- or Pyth-owned semantic release preimage.
    pub semantic_preimage: &'a [u8],
    /// Exact Loader V3 Program account data, without account metadata.
    pub program_account_data: &'a [u8],
    /// Exact Loader V3 ProgramData account data, including its ELF and padding.
    pub programdata_account_data: &'a [u8],
    /// Canonical independently obtained build and account metadata.
    pub metadata: &'a BuildMetadataV1,
}

/// Canonical checked binding between semantic, artifact, loader, and build facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedReleaseV1 {
    semantic_kind: SemanticPreimageKindV1,
    semantic_preimage_len: u64,
    elf_len: u64,
    program_account_len: u64,
    programdata_account_len: u64,
    deployment_slot: u64,
    programdata_elf_offset: u64,
    artifact_digest: [u8; 32],
    semantic_release_id: ContentId,
    program_account_digest: [u8; 32],
    programdata_account_digest: [u8; 32],
    program_id: [u8; 32],
    programdata_id: [u8; 32],
    loader_program_id: [u8; 32],
    upgrade_authority: Option<[u8; 32]>,
    source_digest: [u8; 32],
    cargo_lock_digest: [u8; 32],
    source_revision: String,
    rustc_version: String,
    solana_version: String,
    cargo_build_sbf_version: String,
    target_triple: String,
    build_command: String,
    assumptions: Vec<String>,
}

impl CheckedReleaseV1 {
    /// Hostile-decode one exact canonical checked-release binary manifest.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < CHECKED_RELEASE_FIXED_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(CHECKED_RELEASE_MAGIC_V1.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != CHECKED_RELEASE_SCHEMA_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if bytes.get(RESERVED_OFFSET..MANIFEST_LENGTH_OFFSET) != Some([0_u8; 2].as_slice()) {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let declared_len = usize::try_from(read_u32(bytes, MANIFEST_LENGTH_OFFSET)?)
            .map_err(|_| Error::ArithmeticOverflow)?;
        if declared_len != bytes.len() {
            return Err(Error::InvalidManifestLength);
        }
        let semantic_kind =
            SemanticPreimageKindV1::decode(read_byte(bytes, SEMANTIC_KIND_OFFSET)?)?;
        if read_byte(bytes, LOADER_KIND_OFFSET)? != LOADER_KIND_UPGRADEABLE_V3 {
            return Err(Error::UnknownLoaderKind);
        }
        let authority_kind = read_byte(bytes, AUTHORITY_KIND_OFFSET)?;
        let assumption_count = usize::from(read_byte(bytes, ASSUMPTION_COUNT_OFFSET)?);
        let upgrade_authority_bytes = read_array(bytes, UPGRADE_AUTHORITY_OFFSET)?;
        let upgrade_authority = match authority_kind {
            AUTHORITY_NONE if is_zero(&upgrade_authority_bytes) => None,
            AUTHORITY_NONE => return Err(Error::NonCanonicalUpgradeAuthority),
            AUTHORITY_SOME => {
                require_nonzero(&upgrade_authority_bytes)
                    .map_err(|_| Error::NonCanonicalUpgradeAuthority)?;
                Some(upgrade_authority_bytes)
            }
            _ => return Err(Error::NonCanonicalUpgradeAuthority),
        };
        let semantic_release_id = ContentId::new(read_array(bytes, SEMANTIC_ID_OFFSET)?)
            .map_err(|_| Error::ZeroIdentifier)?;
        let mut decoder = Decoder::new(
            bytes
                .get(CHECKED_RELEASE_FIXED_BYTES_V1..)
                .ok_or(Error::InvalidLength)?,
        );
        let source_revision = decoder.text()?;
        let rustc_version = decoder.text()?;
        let solana_version = decoder.text()?;
        let cargo_build_sbf_version = decoder.text()?;
        let target_triple = decoder.text()?;
        let build_command = decoder.text()?;
        let mut assumptions = Vec::with_capacity(assumption_count);
        for _ in 0..assumption_count {
            assumptions.push(decoder.text()?);
        }
        decoder.finish()?;
        let result = Self {
            semantic_kind,
            semantic_preimage_len: read_u64(bytes, SEMANTIC_LENGTH_OFFSET)?,
            elf_len: read_u64(bytes, ELF_LENGTH_OFFSET)?,
            program_account_len: read_u64(bytes, PROGRAM_LENGTH_OFFSET)?,
            programdata_account_len: read_u64(bytes, PROGRAMDATA_LENGTH_OFFSET)?,
            deployment_slot: read_u64(bytes, DEPLOYMENT_SLOT_OFFSET)?,
            programdata_elf_offset: read_u64(bytes, PROGRAMDATA_ELF_OFFSET_OFFSET)?,
            artifact_digest: read_array(bytes, ARTIFACT_DIGEST_OFFSET)?,
            semantic_release_id,
            program_account_digest: read_array(bytes, PROGRAM_DIGEST_OFFSET)?,
            programdata_account_digest: read_array(bytes, PROGRAMDATA_DIGEST_OFFSET)?,
            program_id: read_array(bytes, PROGRAM_ID_OFFSET)?,
            programdata_id: read_array(bytes, PROGRAMDATA_ID_OFFSET)?,
            loader_program_id: read_array(bytes, LOADER_ID_OFFSET)?,
            upgrade_authority,
            source_digest: read_array(bytes, SOURCE_DIGEST_OFFSET)?,
            cargo_lock_digest: read_array(bytes, CARGO_LOCK_DIGEST_OFFSET)?,
            source_revision,
            rustc_version,
            solana_version,
            cargo_build_sbf_version,
            target_triple,
            build_command,
            assumptions,
        };
        result.validate()?;
        if result.encode()? != bytes {
            return Err(Error::InvalidMetadata);
        }
        Ok(result)
    }

    /// Encode the exact canonical binary manifest.
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let encoded_len = self.encoded_len()?;
        let manifest_len = u32::try_from(encoded_len).map_err(|_| Error::ArithmeticOverflow)?;
        let assumption_count =
            u8::try_from(self.assumptions.len()).map_err(|_| Error::ArithmeticOverflow)?;
        let mut output = Vec::with_capacity(encoded_len);
        output.extend_from_slice(&CHECKED_RELEASE_MAGIC_V1);
        output.extend_from_slice(&CHECKED_RELEASE_SCHEMA_V1.to_le_bytes());
        output.push(self.semantic_kind.byte());
        output.push(LOADER_KIND_UPGRADEABLE_V3);
        output.push(if self.upgrade_authority.is_some() {
            AUTHORITY_SOME
        } else {
            AUTHORITY_NONE
        });
        output.push(assumption_count);
        output.extend_from_slice(&[0; 2]);
        output.extend_from_slice(&manifest_len.to_le_bytes());
        output.extend_from_slice(&self.semantic_preimage_len.to_le_bytes());
        output.extend_from_slice(&self.elf_len.to_le_bytes());
        output.extend_from_slice(&self.program_account_len.to_le_bytes());
        output.extend_from_slice(&self.programdata_account_len.to_le_bytes());
        output.extend_from_slice(&self.deployment_slot.to_le_bytes());
        output.extend_from_slice(&self.programdata_elf_offset.to_le_bytes());
        output.extend_from_slice(&self.artifact_digest);
        output.extend_from_slice(self.semantic_release_id.as_bytes());
        output.extend_from_slice(&self.program_account_digest);
        output.extend_from_slice(&self.programdata_account_digest);
        output.extend_from_slice(&self.program_id);
        output.extend_from_slice(&self.programdata_id);
        output.extend_from_slice(&self.loader_program_id);
        output.extend_from_slice(&self.upgrade_authority.unwrap_or([0; 32]));
        output.extend_from_slice(&self.source_digest);
        output.extend_from_slice(&self.cargo_lock_digest);
        for text in [
            self.source_revision.as_str(),
            self.rustc_version.as_str(),
            self.solana_version.as_str(),
            self.cargo_build_sbf_version.as_str(),
            self.target_triple.as_str(),
            self.build_command.as_str(),
        ] {
            encode_text(&mut output, text)?;
        }
        for assumption in &self.assumptions {
            encode_text(&mut output, assumption)?;
        }
        if output.len() != encoded_len {
            return Err(Error::ArithmeticOverflow);
        }
        Ok(output)
    }

    /// Compute the SHA-256 content identity of the exact checked manifest.
    pub fn checked_release_id(&self) -> Result<ContentId> {
        ContentId::new(sha256(&self.encode()?)).map_err(|_| Error::ZeroIdentifier)
    }

    /// Emit the deterministic line-oriented machine-readable text projection.
    pub fn render_text(&self) -> Result<String> {
        let mut output = String::new();
        push_line(&mut output, "format", "dclutch-checked-release-v1");
        push_line(
            &mut output,
            "checked_release_id",
            &encode_hex(self.checked_release_id()?.as_bytes()),
        );
        push_line(
            &mut output,
            "manifest_bytes",
            &self.encoded_len()?.to_string(),
        );
        push_line(&mut output, "semantic_kind", self.semantic_kind.label());
        push_line(
            &mut output,
            "semantic_release_id",
            &encode_hex(self.semantic_release_id.as_bytes()),
        );
        push_line(
            &mut output,
            "semantic_preimage_bytes",
            &self.semantic_preimage_len.to_string(),
        );
        push_line(
            &mut output,
            "artifact_sha256",
            &encode_hex(&self.artifact_digest),
        );
        push_line(&mut output, "artifact_bytes", &self.elf_len.to_string());
        push_line(&mut output, "elf_class", "ELF64");
        push_line(&mut output, "elf_endianness", "little");
        push_line(&mut output, "elf_type", "shared-object");
        push_line(&mut output, "elf_machine", "BPF-SBF");
        push_line(&mut output, "loader_profile", "upgradeable-loader-v3");
        push_line(&mut output, "program_id", &encode_hex(&self.program_id));
        push_line(
            &mut output,
            "programdata_id",
            &encode_hex(&self.programdata_id),
        );
        push_line(
            &mut output,
            "loader_program_id",
            &encode_hex(&self.loader_program_id),
        );
        push_line(&mut output, "program_owner_is_loader", "true");
        push_line(&mut output, "program_executable", "true");
        push_line(&mut output, "programdata_owner_is_loader", "true");
        push_line(&mut output, "programdata_executable", "false");
        push_line(
            &mut output,
            "deployment_slot",
            &self.deployment_slot.to_string(),
        );
        let authority = self
            .upgrade_authority
            .map_or_else(|| "none".to_owned(), |value| encode_hex(&value));
        push_line(&mut output, "upgrade_authority", &authority);
        push_line(
            &mut output,
            "program_account_sha256",
            &encode_hex(&self.program_account_digest),
        );
        push_line(
            &mut output,
            "program_account_bytes",
            &self.program_account_len.to_string(),
        );
        push_line(
            &mut output,
            "programdata_account_sha256",
            &encode_hex(&self.programdata_account_digest),
        );
        push_line(
            &mut output,
            "programdata_account_bytes",
            &self.programdata_account_len.to_string(),
        );
        push_line(
            &mut output,
            "programdata_elf_offset",
            &self.programdata_elf_offset.to_string(),
        );
        push_line(
            &mut output,
            "source_digest",
            &encode_hex(&self.source_digest),
        );
        push_line(
            &mut output,
            "cargo_lock_digest",
            &encode_hex(&self.cargo_lock_digest),
        );
        push_line(&mut output, "source_revision", &self.source_revision);
        push_line(&mut output, "rustc_version", &self.rustc_version);
        push_line(&mut output, "solana_version", &self.solana_version);
        push_line(
            &mut output,
            "cargo_build_sbf_version",
            &self.cargo_build_sbf_version,
        );
        push_line(&mut output, "target_triple", &self.target_triple);
        push_line(&mut output, "build_command", &self.build_command);
        for assumption in &self.assumptions {
            push_line(&mut output, "assumption", assumption);
        }
        Ok(output)
    }

    /// Return the existing semantic release identity.
    pub const fn semantic_release_id(&self) -> ContentId {
        self.semantic_release_id
    }

    /// Return the digest of the exact checked ELF bytes.
    pub const fn artifact_digest(&self) -> [u8; 32] {
        self.artifact_digest
    }

    /// Return the Loader V3 deployment slot from ProgramData.
    pub const fn deployment_slot(&self) -> u64 {
        self.deployment_slot
    }

    /// Return the optional Loader V3 upgrade authority.
    pub const fn upgrade_authority(&self) -> Option<[u8; 32]> {
        self.upgrade_authority
    }

    /// Return the exact deployed Program identity.
    pub const fn program_id(&self) -> [u8; 32] {
        self.program_id
    }

    /// Return the exact deployed ProgramData identity.
    pub const fn programdata_id(&self) -> [u8; 32] {
        self.programdata_id
    }

    /// Return the exact Loader V3 program identity.
    pub const fn loader_program_id(&self) -> [u8; 32] {
        self.loader_program_id
    }

    fn validate(&self) -> Result<()> {
        if self.semantic_preimage_len == 0
            || self.elf_len
                < u64::try_from(ELF_HEADER_BYTES).map_err(|_| Error::ArithmeticOverflow)?
            || self.program_account_len
                != u64::try_from(LOADER_V3_PROGRAM_BYTES).map_err(|_| Error::ArithmeticOverflow)?
            || self.programdata_elf_offset
                != u64::try_from(LOADER_V3_PROGRAMDATA_METADATA_BYTES)
                    .map_err(|_| Error::ArithmeticOverflow)?
        {
            return Err(Error::InvalidLength);
        }
        if self.elf_len
            > u64::try_from(LOADER_V3_MAX_ELF_BYTES).map_err(|_| Error::ArithmeticOverflow)?
        {
            return Err(Error::ArtifactExceedsLoaderLimit);
        }
        if self.programdata_account_len
            > u64::try_from(SOLANA_MAX_PERMITTED_ACCOUNT_DATA_BYTES)
                .map_err(|_| Error::ArithmeticOverflow)?
        {
            return Err(Error::ProgramDataExceedsAccountLimit);
        }
        let required_programdata = self
            .programdata_elf_offset
            .checked_add(self.elf_len)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.programdata_account_len < required_programdata {
            return Err(Error::ProgramDataTooShort);
        }
        for value in [
            self.artifact_digest,
            self.program_account_digest,
            self.programdata_account_digest,
            self.program_id,
            self.programdata_id,
            self.loader_program_id,
            self.source_digest,
            self.cargo_lock_digest,
        ] {
            require_nonzero(&value)?;
        }
        if self.program_id == self.programdata_id
            || self.program_id == self.loader_program_id
            || self.programdata_id == self.loader_program_id
        {
            return Err(Error::AliasedLoaderIdentity);
        }
        if let Some(authority) = self.upgrade_authority {
            require_nonzero(&authority).map_err(|_| Error::NonCanonicalUpgradeAuthority)?;
        }
        for text in [
            self.source_revision.as_str(),
            self.rustc_version.as_str(),
            self.solana_version.as_str(),
            self.cargo_build_sbf_version.as_str(),
            self.target_triple.as_str(),
            self.build_command.as_str(),
        ] {
            validate_text(text)?;
        }
        validate_assumptions(&self.assumptions)
    }

    fn encoded_len(&self) -> Result<usize> {
        let mut total = CHECKED_RELEASE_FIXED_BYTES_V1;
        for text in [
            self.source_revision.as_str(),
            self.rustc_version.as_str(),
            self.solana_version.as_str(),
            self.cargo_build_sbf_version.as_str(),
            self.target_triple.as_str(),
            self.build_command.as_str(),
        ] {
            total = checked_text_len(total, text)?;
        }
        for assumption in &self.assumptions {
            total = checked_text_len(total, assumption)?;
        }
        let _ = u32::try_from(total).map_err(|_| Error::ArithmeticOverflow)?;
        Ok(total)
    }
}

/// Construct a checked release from exact offline evidence.
pub fn build_checked_release(evidence: ReleaseEvidenceV1<'_>) -> Result<CheckedReleaseV1> {
    evidence.metadata.validate()?;
    if evidence.semantic_preimage.is_empty() || evidence.elf.is_empty() {
        return Err(Error::EmptyInput);
    }
    validate_semantic_preimage(evidence.metadata.semantic_kind, evidence.semantic_preimage)?;
    validate_sbf_elf(evidence.elf)?;
    if evidence.programdata_account_data.len() > SOLANA_MAX_PERMITTED_ACCOUNT_DATA_BYTES {
        return Err(Error::ProgramDataExceedsAccountLimit);
    }
    let program = ProgramV3View::parse(evidence.program_account_data)?;
    if program.programdata_key() != evidence.metadata.programdata_id {
        return Err(Error::ProgramDataLinkMismatch);
    }
    let programdata = ProgramDataV3View::parse(evidence.programdata_account_data)?;
    let payload = evidence
        .programdata_account_data
        .get(LOADER_V3_PROGRAMDATA_METADATA_BYTES..)
        .ok_or(Error::ProgramDataTooShort)?;
    let deployed_elf = payload
        .get(..evidence.elf.len())
        .ok_or(Error::ProgramDataTooShort)?;
    if deployed_elf != evidence.elf {
        return Err(Error::DeployedElfMismatch);
    }
    let padding = payload
        .get(evidence.elf.len()..)
        .ok_or(Error::ProgramDataTooShort)?;
    if padding.iter().any(|byte| *byte != 0) {
        return Err(Error::NonZeroProgramDataPadding);
    }
    let semantic_preimage_len =
        u64::try_from(evidence.semantic_preimage.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let elf_len = u64::try_from(evidence.elf.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let program_account_len = u64::try_from(evidence.program_account_data.len())
        .map_err(|_| Error::ArithmeticOverflow)?;
    let programdata_account_len = u64::try_from(evidence.programdata_account_data.len())
        .map_err(|_| Error::ArithmeticOverflow)?;
    let programdata_elf_offset = u64::try_from(LOADER_V3_PROGRAMDATA_METADATA_BYTES)
        .map_err(|_| Error::ArithmeticOverflow)?;
    let semantic_release_id =
        ContentId::new(sha256(evidence.semantic_preimage)).map_err(|_| Error::ZeroIdentifier)?;
    let result = CheckedReleaseV1 {
        semantic_kind: evidence.metadata.semantic_kind,
        semantic_preimage_len,
        elf_len,
        program_account_len,
        programdata_account_len,
        deployment_slot: programdata.deployment_slot(),
        programdata_elf_offset,
        artifact_digest: sha256(evidence.elf),
        semantic_release_id,
        program_account_digest: sha256(evidence.program_account_data),
        programdata_account_digest: sha256(evidence.programdata_account_data),
        program_id: evidence.metadata.program_id,
        programdata_id: evidence.metadata.programdata_id,
        loader_program_id: evidence.metadata.loader_program_id,
        upgrade_authority: programdata.upgrade_authority(),
        source_digest: evidence.metadata.source_digest,
        cargo_lock_digest: evidence.metadata.cargo_lock_digest,
        source_revision: evidence.metadata.source_revision.clone(),
        rustc_version: evidence.metadata.rustc_version.clone(),
        solana_version: evidence.metadata.solana_version.clone(),
        cargo_build_sbf_version: evidence.metadata.cargo_build_sbf_version.clone(),
        target_triple: evidence.metadata.target_triple.clone(),
        build_command: evidence.metadata.build_command.clone(),
        assumptions: evidence.metadata.assumptions.clone(),
    };
    result.validate()?;
    Ok(result)
}

/// Decode a checked manifest and require exact equality with rebuilt evidence.
pub fn verify_checked_release(
    manifest_bytes: &[u8],
    evidence: ReleaseEvidenceV1<'_>,
) -> Result<CheckedReleaseV1> {
    let checked = CheckedReleaseV1::decode(manifest_bytes)?;
    let rebuilt = build_checked_release(evidence)?;
    if checked != rebuilt {
        return Err(Error::CheckedManifestMismatch);
    }
    Ok(checked)
}

fn validate_semantic_preimage(kind: SemanticPreimageKindV1, bytes: &[u8]) -> Result<()> {
    match kind {
        SemanticPreimageKindV1::Capability => {
            if bytes.is_empty() {
                return Err(Error::EmptyInput);
            }
        }
        SemanticPreimageKindV1::PythReleaseV1 => {
            let _ = PythReleaseV1::decode(bytes).map_err(Error::InvalidPythRelease)?;
        }
    }
    Ok(())
}

fn validate_sbf_elf(bytes: &[u8]) -> Result<()> {
    if bytes.len() > LOADER_V3_MAX_ELF_BYTES {
        return Err(Error::ArtifactExceedsLoaderLimit);
    }
    if bytes.len() < ELF_HEADER_BYTES {
        return Err(Error::InvalidSbfElf);
    }
    let machine = read_u16(bytes, 18)?;
    if bytes.get(..4) != Some([0x7f, b'E', b'L', b'F'].as_slice())
        || read_byte(bytes, 4)? != ELF_CLASS_64
        || read_byte(bytes, 5)? != ELF_DATA_LITTLE_ENDIAN
        || read_byte(bytes, 6)? != ELF_CURRENT_VERSION
        || read_u16(bytes, 16)? != ELF_TYPE_SHARED_OBJECT
        || (machine != ELF_MACHINE_BPF && machine != ELF_MACHINE_SBF)
        || read_u32(bytes, 20)? != u32::from(ELF_CURRENT_VERSION)
        || read_u16(bytes, 52)? != 64
    {
        return Err(Error::InvalidSbfElf);
    }
    Ok(())
}

fn value<'a>(lines: &mut impl Iterator<Item = &'a str>, prefix: &str) -> Result<&'a str> {
    lines
        .next()
        .and_then(|line| line.strip_prefix(prefix))
        .ok_or(Error::InvalidMetadata)
}

fn parse_bool(value: &str) -> Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(Error::InvalidBoolean),
    }
}

fn validate_text(value: &str) -> Result<()> {
    if value.is_empty()
        || u16::try_from(value.len()).is_err()
        || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return Err(Error::InvalidText);
    }
    Ok(())
}

fn validate_assumptions(assumptions: &[String]) -> Result<()> {
    if assumptions.is_empty() || u8::try_from(assumptions.len()).is_err() {
        return Err(Error::NonCanonicalAssumptions);
    }
    let mut previous: Option<&str> = None;
    for assumption in assumptions {
        validate_text(assumption)?;
        if previous.is_some_and(|prior| prior >= assumption.as_str()) {
            return Err(Error::NonCanonicalAssumptions);
        }
        previous = Some(assumption);
    }
    Ok(())
}

fn require_nonzero(value: &[u8; 32]) -> Result<()> {
    if is_zero(value) {
        return Err(Error::ZeroIdentifier);
    }
    Ok(())
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn checked_text_len(current: usize, text: &str) -> Result<usize> {
    validate_text(text)?;
    current
        .checked_add(2)
        .and_then(|value| value.checked_add(text.len()))
        .ok_or(Error::ArithmeticOverflow)
}

fn encode_text(output: &mut Vec<u8>, text: &str) -> Result<()> {
    validate_text(text)?;
    let length = u16::try_from(text.len()).map_err(|_| Error::ArithmeticOverflow)?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(text.as_bytes());
    Ok(())
}

fn decode_hex_32(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(Error::InvalidHex);
    }
    let mut output = [0_u8; 32];
    for (destination, pair) in output.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let high = decode_nibble(*pair.first().ok_or(Error::InvalidHex)?)?;
        let low = decode_nibble(*pair.get(1).ok_or(Error::InvalidHex)?)?;
        *destination = high
            .checked_mul(16)
            .and_then(|part| part.checked_add(low))
            .ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(output)
}

fn decode_nibble(value: u8) -> Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(Error::InvalidHex),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(hex_digit(*byte >> 4));
        output.push(hex_digit(*byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + (value - 10)),
        _ => '?',
    }
}

fn push_line(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn text(&mut self) -> Result<String> {
        let length = usize::from(u16::from_le_bytes(self.array()?));
        let end = self
            .offset
            .checked_add(length)
            .ok_or(Error::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Error::InvalidLength)?;
        self.offset = end;
        let text = str::from_utf8(value).map_err(|_| Error::InvalidText)?;
        validate_text(text)?;
        Ok(text.to_owned())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(Error::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Error::InvalidLength)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?;
        self.offset = end;
        Ok(value)
    }

    fn finish(self) -> Result<()> {
        if self.offset != self.bytes.len() {
            return Err(Error::InvalidMetadata);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
