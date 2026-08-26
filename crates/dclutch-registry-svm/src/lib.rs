#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact SDK-free byte views at the successor Registry SBF boundary.
//!
//! This crate owns no authority. It only hostile-decodes the fixed instruction
//! and authenticated-receipt wires and the Loader V3 byte layouts consumed by
//! the Registry adapter. Solana account identity, ownership, executable flags,
//! PDA derivation, hashing, CPI, and return-data provenance remain adapter
//! obligations.

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{ArtifactReleaseIdV1, ExecutionRoleV1, ProgramIdentityV1};

/// Canonical family-neutral batched role authentication wires.
pub mod batch_v2;
/// Invocation-scoped Registry-authenticated continuation wires.
pub mod continuation_v1;

/// Exact Upgradeable Loader V3 Program account-data width.
pub const LOADER_V3_PROGRAM_BYTES: usize = 36;
/// Fixed Upgradeable Loader V3 ProgramData metadata allocation width.
pub const LOADER_V3_PROGRAMDATA_METADATA_BYTES: usize = 45;
/// Exact Registry instruction width.
pub const REGISTRY_INSTRUCTION_BYTES_V1: usize = 16;
/// Exact authenticated-role receipt width.
pub const AUTHENTICATED_ROLE_RECEIPT_BYTES_V1: usize = 144;
/// Registry instruction magic.
pub const REGISTRY_INSTRUCTION_MAGIC_V1: [u8; 8] = *b"DCLTRIX1";
/// Authenticated-role receipt magic.
pub const AUTHENTICATED_ROLE_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTRRR1";
/// Implemented Registry wire schema.
pub const REGISTRY_WIRE_SCHEMA_V1: u16 = 1;

const SCHEMA_OFFSET: usize = 8;
const ACTION_OFFSET: usize = 10;
const ROLE_OFFSET: usize = 11;
const INSTRUCTION_RESERVED_OFFSET: usize = 12;
const RECEIPT_RESERVED_OFFSET: usize = 11;
const RELEASE_SET_OFFSET: usize = 16;
const PROGRAM_OFFSET: usize = 48;
const ARTIFACT_RELEASE_OFFSET: usize = 80;
const SEMANTIC_RELEASE_OFFSET: usize = 112;

/// Stable refusal from an exact Registry SVM byte view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The input did not have its one exact required width.
    InvalidLength,
    /// Magic bytes selected a different wire family.
    InvalidMagic,
    /// The requested wire schema is unsupported.
    UnsupportedSchema,
    /// An instruction action discriminator is unknown.
    UnknownAction,
    /// A role discriminator is outside the five-role profile.
    UnknownRole,
    /// Reserved or action-inactive bytes were not zero.
    NonCanonicalReservedBytes,
    /// Loader state named an unsupported enum variant.
    InvalidLoaderVariant,
    /// Loader ProgramData named an invalid `Option<Pubkey>` tag.
    InvalidUpgradeAuthorityTag,
    /// ProgramData contained no byte after its fixed metadata allocation.
    EmptyElf,
    /// A typed execution-release-set identity refused hostile bytes.
    ReleaseSet(dclutch_release_set_contract::Error),
    /// A typed content identity refused hostile bytes.
    Content(dclutch_core_contract::Error),
}

impl From<dclutch_release_set_contract::Error> for Error {
    fn from(value: dclutch_release_set_contract::Error) -> Self {
        Self::ReleaseSet(value)
    }
}

impl From<dclutch_core_contract::Error> for Error {
    fn from(value: dclutch_core_contract::Error) -> Self {
        Self::Content(value)
    }
}

/// Result alias for Registry SVM wire operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact Registry instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryInstructionV1 {
    /// Authenticate five finalized artifact releases and create their cache.
    Activate,
    /// Reauthenticate one cached role against its current Loader deployment.
    Reauthenticate(ExecutionRoleV1),
}

impl RegistryInstructionV1 {
    /// Hostile-decode one exact Registry instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            &REGISTRY_INSTRUCTION_MAGIC_V1,
            REGISTRY_INSTRUCTION_BYTES_V1,
        )?;
        require_zero(bytes, INSTRUCTION_RESERVED_OFFSET, 4)?;
        let action = read_byte(bytes, ACTION_OFFSET)?;
        let role = read_byte(bytes, ROLE_OFFSET)?;
        match action {
            0 if role == 0 => Ok(Self::Activate),
            0 => Err(Error::NonCanonicalReservedBytes),
            1 => Ok(Self::Reauthenticate(decode_role(role)?)),
            _ => Err(Error::UnknownAction),
        }
    }

    /// Encode the one canonical Registry instruction.
    pub fn to_bytes(self) -> [u8; REGISTRY_INSTRUCTION_BYTES_V1] {
        let mut output = [0_u8; REGISTRY_INSTRUCTION_BYTES_V1];
        copy(&mut output, 0, &REGISTRY_INSTRUCTION_MAGIC_V1);
        copy(
            &mut output,
            SCHEMA_OFFSET,
            &REGISTRY_WIRE_SCHEMA_V1.to_le_bytes(),
        );
        match self {
            Self::Activate => {}
            Self::Reauthenticate(role) => {
                set(&mut output, ACTION_OFFSET, 1);
                set(&mut output, ROLE_OFFSET, role_byte(role));
            }
        }
        output
    }
}

/// CPI return data proving one Registry role was reauthenticated this call.
///
/// A consumer must additionally require that Solana reports the expected
/// Registry program as the return-data producer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRoleReceiptV1 {
    role: ExecutionRoleV1,
    execution_release_set_id: ContentId,
    program: ProgramIdentityV1,
    artifact_release_id: ArtifactReleaseIdV1,
    semantic_release_id: ContentId,
}

impl AuthenticatedRoleReceiptV1 {
    /// Construct one receipt from already authenticated Registry facts.
    pub const fn new(
        role: ExecutionRoleV1,
        execution_release_set_id: ContentId,
        program: ProgramIdentityV1,
        artifact_release_id: ArtifactReleaseIdV1,
        semantic_release_id: ContentId,
    ) -> Self {
        Self {
            role,
            execution_release_set_id,
            program,
            artifact_release_id,
            semantic_release_id,
        }
    }

    /// Hostile-decode one exact authenticated-role receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            &AUTHENTICATED_ROLE_RECEIPT_MAGIC_V1,
            AUTHENTICATED_ROLE_RECEIPT_BYTES_V1,
        )?;
        require_zero(bytes, RECEIPT_RESERVED_OFFSET, 5)?;
        Ok(Self::new(
            decode_role(read_byte(bytes, ACTION_OFFSET)?)?,
            ContentId::decode(slice(bytes, RELEASE_SET_OFFSET, 32)?)?,
            ProgramIdentityV1::decode(slice(bytes, PROGRAM_OFFSET, 32)?)?,
            ArtifactReleaseIdV1::decode(slice(bytes, ARTIFACT_RELEASE_OFFSET, 32)?)?,
            ContentId::decode(slice(bytes, SEMANTIC_RELEASE_OFFSET, 32)?)?,
        ))
    }

    /// Encode the one canonical receipt.
    pub fn to_bytes(self) -> [u8; AUTHENTICATED_ROLE_RECEIPT_BYTES_V1] {
        let mut output = [0_u8; AUTHENTICATED_ROLE_RECEIPT_BYTES_V1];
        copy(&mut output, 0, &AUTHENTICATED_ROLE_RECEIPT_MAGIC_V1);
        copy(
            &mut output,
            SCHEMA_OFFSET,
            &REGISTRY_WIRE_SCHEMA_V1.to_le_bytes(),
        );
        set(&mut output, ACTION_OFFSET, role_byte(self.role));
        copy(
            &mut output,
            RELEASE_SET_OFFSET,
            self.execution_release_set_id.as_bytes(),
        );
        copy(&mut output, PROGRAM_OFFSET, self.program.as_bytes());
        copy(
            &mut output,
            ARTIFACT_RELEASE_OFFSET,
            self.artifact_release_id.as_bytes(),
        );
        copy(
            &mut output,
            SEMANTIC_RELEASE_OFFSET,
            self.semantic_release_id.as_bytes(),
        );
        output
    }

    /// Return the authenticated role.
    pub const fn role(self) -> ExecutionRoleV1 {
        self.role
    }

    /// Return the authenticated execution-release-set identity.
    pub const fn execution_release_set_id(self) -> ContentId {
        self.execution_release_set_id
    }

    /// Return the role's authenticated program identity.
    pub const fn program(self) -> ProgramIdentityV1 {
        self.program
    }

    /// Return the role's authenticated artifact-release identity.
    pub const fn artifact_release_id(self) -> ArtifactReleaseIdV1 {
        self.artifact_release_id
    }

    /// Return the role's authenticated semantic-release identity.
    pub const fn semantic_release_id(self) -> ContentId {
        self.semantic_release_id
    }
}

/// Exact Loader V3 Program account-data view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramV3View {
    programdata: [u8; 32],
}

impl ProgramV3View {
    /// Decode exact variant-two Loader V3 Program bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != LOADER_V3_PROGRAM_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_u32(bytes, 0)? != 2 {
            return Err(Error::InvalidLoaderVariant);
        }
        Ok(Self {
            programdata: read_array(bytes, 4)?,
        })
    }

    /// Return the exact linked ProgramData identity.
    pub const fn programdata(self) -> [u8; 32] {
        self.programdata
    }

    /// Return the exact linked ProgramData identity.
    ///
    /// This spelling is retained for consumers that previously used the
    /// parallel Pyth-owned Loader view.
    pub const fn programdata_key(self) -> [u8; 32] {
        self.programdata
    }
}

/// Exact Loader V3 ProgramData metadata and complete deployed ELF-tail view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramDataV3View<'a> {
    deployment_slot: u64,
    upgrade_authority: Option<[u8; 32]>,
    elf: &'a [u8],
}

impl<'a> ProgramDataV3View<'a> {
    /// Decode variant-three ProgramData at Loader V3's fixed 45-byte ELF offset.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() <= LOADER_V3_PROGRAMDATA_METADATA_BYTES {
            return Err(Error::EmptyElf);
        }
        if read_u32(bytes, 0)? != 3 {
            return Err(Error::InvalidLoaderVariant);
        }
        let upgrade_authority = match read_byte(bytes, 12)? {
            // Loader-v3 serializes the shorter `None` state into the existing
            // 45-byte metadata region without clearing bytes 13..45. Those
            // bytes may therefore retain the former authority, but tag zero
            // makes them inactive and they must never be exposed as one.
            0 => None,
            1 => Some(read_array(bytes, 13)?),
            _ => return Err(Error::InvalidUpgradeAuthorityTag),
        };
        Ok(Self {
            deployment_slot: read_u64(bytes, 4)?,
            upgrade_authority,
            elf: slice(
                bytes,
                LOADER_V3_PROGRAMDATA_METADATA_BYTES,
                bytes.len() - LOADER_V3_PROGRAMDATA_METADATA_BYTES,
            )?,
        })
    }

    /// Return the last deployment slot.
    pub const fn deployment_slot(self) -> u64 {
        self.deployment_slot
    }

    /// Return the current optional upgrade authority.
    pub const fn upgrade_authority(self) -> Option<[u8; 32]> {
        self.upgrade_authority
    }

    /// Borrow the complete ProgramData byte tail beginning at fixed offset 45.
    pub const fn elf(self) -> &'a [u8] {
        self.elf
    }
}

fn require_header(bytes: &[u8], magic: &[u8; 8], width: usize) -> Result<()> {
    if bytes.len() != width {
        return Err(Error::InvalidLength);
    }
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, SCHEMA_OFFSET)? != REGISTRY_WIRE_SCHEMA_V1 {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}

fn decode_role(value: u8) -> Result<ExecutionRoleV1> {
    match value {
        0 => Ok(ExecutionRoleV1::Core),
        1 => Ok(ExecutionRoleV1::Claims),
        2 => Ok(ExecutionRoleV1::Trading),
        3 => Ok(ExecutionRoleV1::Resolution),
        4 => Ok(ExecutionRoleV1::Custody),
        _ => Err(Error::UnknownRole),
    }
}

const fn role_byte(role: ExecutionRoleV1) -> u8 {
    match role {
        ExecutionRoleV1::Core => 0,
        ExecutionRoleV1::Claims => 1,
        ExecutionRoleV1::Trading => 2,
        ExecutionRoleV1::Resolution => 3,
        ExecutionRoleV1::Custody => 4,
    }
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_small_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_small_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_small_array(bytes, offset)?))
}

fn read_array(bytes: &[u8], offset: usize) -> Result<[u8; 32]> {
    read_small_array(bytes, offset)
}

fn read_small_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn copy(output: &mut [u8], offset: usize, source: &[u8]) {
    let Some(end) = offset.checked_add(source.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(source);
}

fn set(output: &mut [u8], offset: usize, value: u8) {
    let Some(destination) = output.get_mut(offset) else {
        return;
    };
    *destination = value;
}

#[cfg(test)]
mod tests;
