//! Canonical artifact-release facts and current deployment observations.

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::ProgramIdentityV1;

use crate::{
    Error, IDENTITY_BYTES, Result, copy_infallible, put_u16, put_u64, read_array, read_byte,
    read_u16, read_u64, require_nonzero, require_zero, subslice,
};

/// Exact bytes in one canonical artifact-release record.
pub const ARTIFACT_RELEASE_BYTES_V1: usize = 216;
/// Canonical artifact-release wire magic.
pub const ARTIFACT_RELEASE_MAGIC_V1: [u8; 8] = *b"DCLTARF1";
/// Implemented artifact-release schema.
pub const ARTIFACT_RELEASE_SCHEMA_VERSION_V1: u16 = 1;
/// Implemented artifact-release fixed-layout profile.
pub const ARTIFACT_RELEASE_PROFILE_V1: u16 = 1;
/// Schema/validator identity for artifact-release records.
///
/// This is SHA-256 of `dclutch/schema/artifact-release-v1`.
pub const ARTIFACT_RELEASE_SCHEMA_ID_V1: [u8; IDENTITY_BYTES] = [
    0xae, 0x19, 0xa6, 0x0d, 0xb5, 0x50, 0xb1, 0xa8, 0xa5, 0x1d, 0x46, 0x18, 0xc7, 0x7d, 0xea, 0x54,
    0x21, 0x17, 0x4a, 0x2a, 0x85, 0x5e, 0xe6, 0x77, 0x89, 0x4f, 0xa9, 0x1b, 0x3c, 0xfd, 0x3b, 0x6c,
];

const SCHEMA_OFFSET: usize = 8;
const PROFILE_OFFSET: usize = 10;
const UPGRADE_POLICY_OFFSET: usize = 12;
const HEADER_RESERVED_OFFSET: usize = 13;
const HEADER_RESERVED_BYTES: usize = 3;
const PROGRAM_OFFSET: usize = 16;
const LOADER_OFFSET: usize = 48;
const PROGRAMDATA_OFFSET: usize = 80;
const SEMANTIC_RELEASE_OFFSET: usize = 112;
const ELF_DIGEST_OFFSET: usize = 144;
const DEPLOYMENT_SLOT_OFFSET: usize = 176;
const UPGRADE_AUTHORITY_OFFSET: usize = 184;

/// Upgrade-authority policy admitted by an artifact release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ArtifactUpgradePolicyV1 {
    /// ProgramData has no upgrade authority.
    Immutable = 0,
    /// ProgramData has the one exact nonzero named authority.
    ExactAuthority = 1,
}

impl ArtifactUpgradePolicyV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Immutable => 0,
            Self::ExactAuthority => 1,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Immutable),
            1 => Ok(Self::ExactAuthority),
            _ => Err(Error::NonCanonicalUpgradeAuthority),
        }
    }
}

/// Immutable compact projection of one fully checked executable artifact.
///
/// This record is the sole onchain artifact-release authority.  Reproducible
/// build manifests are evidence used to construct it, not a second runtime
/// admission path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactReleaseV1 {
    program: ProgramIdentityV1,
    loader_program: ProgramIdentityV1,
    programdata: [u8; IDENTITY_BYTES],
    semantic_release_id: ContentId,
    elf_digest: [u8; IDENTITY_BYTES],
    deployment_slot: u64,
    upgrade_policy: ArtifactUpgradePolicyV1,
    upgrade_authority: Option<[u8; IDENTITY_BYTES]>,
}

impl ArtifactReleaseV1 {
    /// Construct and validate one compact artifact release.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        program: ProgramIdentityV1,
        loader_program: ProgramIdentityV1,
        programdata: [u8; IDENTITY_BYTES],
        semantic_release_id: ContentId,
        elf_digest: [u8; IDENTITY_BYTES],
        deployment_slot: u64,
        upgrade_policy: ArtifactUpgradePolicyV1,
        upgrade_authority: Option<[u8; IDENTITY_BYTES]>,
    ) -> Result<Self> {
        require_nonzero(&programdata)?;
        require_nonzero(&elf_digest)?;
        if program.to_bytes() == loader_program.to_bytes()
            || program.to_bytes() == programdata
            || loader_program.to_bytes() == programdata
        {
            return Err(Error::AliasedLoaderIdentity);
        }
        validate_upgrade(upgrade_policy, upgrade_authority)?;
        Ok(Self {
            program,
            loader_program,
            programdata,
            semantic_release_id,
            elf_digest,
            deployment_slot,
            upgrade_policy,
            upgrade_authority,
        })
    }

    /// Hostile-decode one exact canonical artifact release.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ARTIFACT_RELEASE_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..ARTIFACT_RELEASE_MAGIC_V1.len())
            != Some(ARTIFACT_RELEASE_MAGIC_V1.as_slice())
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != ARTIFACT_RELEASE_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, PROFILE_OFFSET)? != ARTIFACT_RELEASE_PROFILE_V1 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
        let policy = ArtifactUpgradePolicyV1::decode(read_byte(bytes, UPGRADE_POLICY_OFFSET)?)?;
        let authority_bytes = read_array(bytes, UPGRADE_AUTHORITY_OFFSET)?;
        let authority = match policy {
            ArtifactUpgradePolicyV1::Immutable => {
                if authority_bytes != [0; IDENTITY_BYTES] {
                    return Err(Error::NonCanonicalUpgradeAuthority);
                }
                None
            }
            ArtifactUpgradePolicyV1::ExactAuthority => Some(authority_bytes),
        };
        Self::new(
            ProgramIdentityV1::decode(subslice(bytes, PROGRAM_OFFSET, IDENTITY_BYTES)?)?,
            ProgramIdentityV1::decode(subslice(bytes, LOADER_OFFSET, IDENTITY_BYTES)?)?,
            read_array(bytes, PROGRAMDATA_OFFSET)?,
            ContentId::new(read_array(bytes, SEMANTIC_RELEASE_OFFSET)?)
                .map_err(|_| Error::ZeroIdentity)?,
            read_array(bytes, ELF_DIGEST_OFFSET)?,
            read_u64(bytes, DEPLOYMENT_SLOT_OFFSET)?,
            policy,
            authority,
        )
    }

    /// Encode the one canonical artifact-release preimage.
    pub fn to_bytes(self) -> [u8; ARTIFACT_RELEASE_BYTES_V1] {
        let mut output = [0; ARTIFACT_RELEASE_BYTES_V1];
        copy_infallible(&mut output, 0, &ARTIFACT_RELEASE_MAGIC_V1);
        put_u16(
            &mut output,
            SCHEMA_OFFSET,
            ARTIFACT_RELEASE_SCHEMA_VERSION_V1,
        );
        put_u16(&mut output, PROFILE_OFFSET, ARTIFACT_RELEASE_PROFILE_V1);
        if let Some(policy) = output.get_mut(UPGRADE_POLICY_OFFSET) {
            *policy = self.upgrade_policy.byte();
        }
        copy_infallible(&mut output, PROGRAM_OFFSET, self.program.as_bytes());
        copy_infallible(&mut output, LOADER_OFFSET, self.loader_program.as_bytes());
        copy_infallible(&mut output, PROGRAMDATA_OFFSET, &self.programdata);
        copy_infallible(
            &mut output,
            SEMANTIC_RELEASE_OFFSET,
            self.semantic_release_id.as_bytes(),
        );
        copy_infallible(&mut output, ELF_DIGEST_OFFSET, &self.elf_digest);
        put_u64(&mut output, DEPLOYMENT_SLOT_OFFSET, self.deployment_slot);
        if let Some(authority) = self.upgrade_authority {
            copy_infallible(&mut output, UPGRADE_AUTHORITY_OFFSET, &authority);
        }
        output
    }

    /// Return the exact executable program identity.
    pub const fn program(self) -> ProgramIdentityV1 {
        self.program
    }

    /// Return the exact Loader program identity.
    pub const fn loader_program(self) -> ProgramIdentityV1 {
        self.loader_program
    }

    /// Return the exact ProgramData identity.
    pub const fn programdata(self) -> [u8; IDENTITY_BYTES] {
        self.programdata
    }

    /// Return the semantic release implemented by this artifact.
    pub const fn semantic_release_id(self) -> ContentId {
        self.semantic_release_id
    }

    /// Return the digest of the complete admitted ELF.
    pub const fn elf_digest(self) -> [u8; IDENTITY_BYTES] {
        self.elf_digest
    }

    /// Return the exact admitted ProgramData deployment slot.
    pub const fn deployment_slot(self) -> u64 {
        self.deployment_slot
    }

    /// Return the admitted upgrade policy.
    pub const fn upgrade_policy(self) -> ArtifactUpgradePolicyV1 {
        self.upgrade_policy
    }

    /// Return the exact upgrade authority, if the release is upgradeable.
    pub const fn upgrade_authority(self) -> Option<[u8; IDENTITY_BYTES]> {
        self.upgrade_authority
    }

    /// Authenticate one current Program/ProgramData/ELF observation.
    pub fn authenticate_deployment(self, observed: DeploymentObservationV1) -> Result<()> {
        if observed.program != self.program.to_bytes()
            || observed.programdata != self.programdata
            || observed.loader_program != self.loader_program.to_bytes()
        {
            return Err(Error::DeploymentIdentityMismatch);
        }
        if observed.programdata_link != self.programdata {
            return Err(Error::ProgramDataLinkMismatch);
        }
        if observed.program_owner != self.loader_program.to_bytes()
            || observed.programdata_owner != self.loader_program.to_bytes()
        {
            return Err(Error::LoaderOwnerMismatch);
        }
        if !observed.program_executable {
            return Err(Error::ProgramNotExecutable);
        }
        if observed.programdata_executable {
            return Err(Error::ProgramDataExecutable);
        }
        if observed.deployment_slot != self.deployment_slot {
            return Err(Error::DeploymentSlotMismatch);
        }
        if observed.elf_digest != self.elf_digest {
            return Err(Error::ElfDigestMismatch);
        }
        if observed.upgrade_authority != self.upgrade_authority {
            return Err(Error::UpgradeAuthorityMismatch);
        }
        Ok(())
    }
}

/// Chain-derived current observation of one Loader V3 deployment.
///
/// An SBF adapter constructs this only after hostile parsing of the actual
/// Program and ProgramData accounts and hashing the exact ELF tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeploymentObservationV1 {
    program: [u8; IDENTITY_BYTES],
    program_owner: [u8; IDENTITY_BYTES],
    program_executable: bool,
    programdata: [u8; IDENTITY_BYTES],
    programdata_owner: [u8; IDENTITY_BYTES],
    programdata_executable: bool,
    programdata_link: [u8; IDENTITY_BYTES],
    loader_program: [u8; IDENTITY_BYTES],
    deployment_slot: u64,
    elf_digest: [u8; IDENTITY_BYTES],
    upgrade_authority: Option<[u8; IDENTITY_BYTES]>,
}

impl DeploymentObservationV1 {
    /// Construct one complete chain-derived observation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        program: [u8; IDENTITY_BYTES],
        program_owner: [u8; IDENTITY_BYTES],
        program_executable: bool,
        programdata: [u8; IDENTITY_BYTES],
        programdata_owner: [u8; IDENTITY_BYTES],
        programdata_executable: bool,
        programdata_link: [u8; IDENTITY_BYTES],
        loader_program: [u8; IDENTITY_BYTES],
        deployment_slot: u64,
        elf_digest: [u8; IDENTITY_BYTES],
        upgrade_authority: Option<[u8; IDENTITY_BYTES]>,
    ) -> Result<Self> {
        for identity in [
            program,
            program_owner,
            programdata,
            programdata_owner,
            programdata_link,
            loader_program,
            elf_digest,
        ] {
            require_nonzero(&identity)?;
        }
        if let Some(authority) = upgrade_authority {
            require_nonzero(&authority).map_err(|_| Error::NonCanonicalUpgradeAuthority)?;
        }
        Ok(Self {
            program,
            program_owner,
            program_executable,
            programdata,
            programdata_owner,
            programdata_executable,
            programdata_link,
            loader_program,
            deployment_slot,
            elf_digest,
            upgrade_authority,
        })
    }
}

fn validate_upgrade(
    policy: ArtifactUpgradePolicyV1,
    authority: Option<[u8; IDENTITY_BYTES]>,
) -> Result<()> {
    match (policy, authority) {
        (ArtifactUpgradePolicyV1::Immutable, None) => Ok(()),
        (ArtifactUpgradePolicyV1::ExactAuthority, Some(value)) => {
            require_nonzero(&value).map_err(|_| Error::NonCanonicalUpgradeAuthority)
        }
        _ => Err(Error::NonCanonicalUpgradeAuthority),
    }
}
