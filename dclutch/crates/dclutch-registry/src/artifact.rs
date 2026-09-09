//! Canonical artifact-release facts and current deployment observations.

use crate::release_set::ProgramIdentityV1;
use dclutch_core_contract::ContentId;

use crate::{
    Error, IDENTITY_BYTES, Result, copy_infallible, put_u16, put_u64, read_array, read_byte,
    read_u16, read_u64, require_nonzero, require_zero, subslice,
};

/// Exact bytes in one canonical artifact-release record.
pub const ARTIFACT_RELEASE_BYTES_V2: usize = 216;
/// Canonical artifact-release wire magic.
pub const ARTIFACT_RELEASE_MAGIC_V2: [u8; 8] = *b"DCLTARF2";
/// Implemented artifact-release schema.
pub const ARTIFACT_RELEASE_SCHEMA_VERSION_V2: u16 = 2;
/// Implemented artifact-release fixed-layout profile.
pub const ARTIFACT_RELEASE_PROFILE_V2: u16 = 1;
/// Schema/validator identity for artifact-release records.
///
/// This is SHA-256 of `dclutch/schema/artifact-release-v2`.
pub const ARTIFACT_RELEASE_SCHEMA_ID_V2: [u8; IDENTITY_BYTES] = [
    0x27, 0xf7, 0x21, 0x9c, 0x3d, 0xd7, 0x14, 0x50, 0x59, 0x13, 0x77, 0xa7, 0x20, 0x17, 0x57, 0x3b,
    0xfd, 0x67, 0xa4, 0x0d, 0x84, 0xd5, 0xdd, 0x61, 0xef, 0xe6, 0xfc, 0xf9, 0x71, 0x6b, 0xa5, 0x9c,
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
const CODE_COMMITMENT_OFFSET: usize = 144;
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
pub struct ArtifactReleaseV2 {
    program: ProgramIdentityV1,
    loader_program: ProgramIdentityV1,
    programdata: [u8; IDENTITY_BYTES],
    semantic_release_id: ContentId,
    code_commitment: [u8; IDENTITY_BYTES],
    deployment_slot: u64,
    upgrade_policy: ArtifactUpgradePolicyV1,
    upgrade_authority: Option<[u8; IDENTITY_BYTES]>,
}

impl ArtifactReleaseV2 {
    /// Construct and validate one compact artifact release.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        program: ProgramIdentityV1,
        loader_program: ProgramIdentityV1,
        programdata: [u8; IDENTITY_BYTES],
        semantic_release_id: ContentId,
        code_commitment: [u8; IDENTITY_BYTES],
        deployment_slot: u64,
        upgrade_policy: ArtifactUpgradePolicyV1,
        upgrade_authority: Option<[u8; IDENTITY_BYTES]>,
    ) -> Result<Self> {
        require_nonzero(&programdata)?;
        require_nonzero(&code_commitment)?;
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
            code_commitment,
            deployment_slot,
            upgrade_policy,
            upgrade_authority,
        })
    }

    /// Hostile-decode one exact canonical artifact release.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ARTIFACT_RELEASE_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..ARTIFACT_RELEASE_MAGIC_V2.len())
            != Some(ARTIFACT_RELEASE_MAGIC_V2.as_slice())
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != ARTIFACT_RELEASE_SCHEMA_VERSION_V2 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, PROFILE_OFFSET)? != ARTIFACT_RELEASE_PROFILE_V2 {
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
            read_array(bytes, CODE_COMMITMENT_OFFSET)?,
            read_u64(bytes, DEPLOYMENT_SLOT_OFFSET)?,
            policy,
            authority,
        )
    }

    /// Encode the one canonical artifact-release preimage.
    pub fn to_bytes(self) -> [u8; ARTIFACT_RELEASE_BYTES_V2] {
        let mut output = [0; ARTIFACT_RELEASE_BYTES_V2];
        copy_infallible(&mut output, 0, &ARTIFACT_RELEASE_MAGIC_V2);
        put_u16(
            &mut output,
            SCHEMA_OFFSET,
            ARTIFACT_RELEASE_SCHEMA_VERSION_V2,
        );
        put_u16(&mut output, PROFILE_OFFSET, ARTIFACT_RELEASE_PROFILE_V2);
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
        copy_infallible(&mut output, CODE_COMMITMENT_OFFSET, &self.code_commitment);
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

    /// Return the canonical ordered commitment to the complete admitted ELF tail.
    pub const fn code_commitment(self) -> [u8; IDENTITY_BYTES] {
        self.code_commitment
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

    /// Authenticate the native Loader coordinates independently of code coverage.
    ///
    /// This proves continuity only. Registry finalization must still verify the
    /// complete code commitment before this record becomes an admission fact.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate_loader_envelope(
        self,
        program: [u8; 32],
        program_owner: [u8; 32],
        program_executable: bool,
        programdata: [u8; 32],
        programdata_owner: [u8; 32],
        programdata_executable: bool,
        programdata_link: [u8; 32],
        loader_program: [u8; 32],
        deployment_slot: u64,
        upgrade_authority: Option<[u8; 32]>,
    ) -> Result<()> {
        if program != self.program.to_bytes()
            || programdata != self.programdata
            || loader_program != self.loader_program.to_bytes()
        {
            return Err(Error::DeploymentIdentityMismatch);
        }
        if programdata_link != self.programdata {
            return Err(Error::ProgramDataLinkMismatch);
        }
        if program_owner != self.loader_program.to_bytes()
            || programdata_owner != self.loader_program.to_bytes()
        {
            return Err(Error::LoaderOwnerMismatch);
        }
        if !program_executable {
            return Err(Error::ProgramNotExecutable);
        }
        if programdata_executable {
            return Err(Error::ProgramDataExecutable);
        }
        if deployment_slot != self.deployment_slot {
            return Err(self.slot_pin_refusal(deployment_slot));
        }
        if upgrade_authority != self.upgrade_authority {
            return Err(Error::UpgradeAuthorityMismatch);
        }
        Ok(())
    }

    /// Authenticate both the native Loader envelope and the complete code commitment.
    pub fn authenticate_deployment(self, observed: DeploymentObservationV2) -> Result<()> {
        self.authenticate_loader_envelope(
            observed.program,
            observed.program_owner,
            observed.program_executable,
            observed.programdata,
            observed.programdata_owner,
            observed.programdata_executable,
            observed.programdata_link,
            observed.loader_program,
            observed.deployment_slot,
            observed.upgrade_authority,
        )?;
        if observed.code_commitment != self.code_commitment {
            return Err(Error::CodeCommitmentMismatch);
        }
        Ok(())
    }

    /// Name a slot mismatch: superseded by an upgrade, or plain staleness.
    ///
    /// An `Immutable` release pins a slot that nothing can move, so any
    /// mismatch is a substituted or wrong-generation observation. An
    /// `ExactAuthority` release pins a slot the named authority CAN move, and
    /// under Loader V3 only forward: `Upgrade` refuses when
    /// `clock.slot == programdata.slot` ("Program was deployed in this block
    /// already"), and so does the `Close` that a redeploy would have to precede
    /// it with. A strictly later observed slot on a slot-pinned release is
    /// therefore exactly one event — the substrate was upgraded — and it gets
    /// its own name so an operator reads a remedy instead of a mystery.
    pub(crate) const fn slot_pin_refusal(self, observed_deployment_slot: u64) -> Error {
        match self.upgrade_policy {
            ArtifactUpgradePolicyV1::ExactAuthority
                if observed_deployment_slot > self.deployment_slot =>
            {
                Error::ReleaseSupersededByUpgrade
            }
            _ => Error::DeploymentSlotMismatch,
        }
    }
}

/// Chain-derived current observation of one Loader V3 deployment.
///
/// An adapter constructs this after parsing the actual Program and ProgramData
/// accounts. The code identity comes from complete native commitment verification
/// or continuity of an authenticated finalized release, never a caller claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeploymentObservationV2 {
    program: [u8; IDENTITY_BYTES],
    program_owner: [u8; IDENTITY_BYTES],
    program_executable: bool,
    programdata: [u8; IDENTITY_BYTES],
    programdata_owner: [u8; IDENTITY_BYTES],
    programdata_executable: bool,
    programdata_link: [u8; IDENTITY_BYTES],
    loader_program: [u8; IDENTITY_BYTES],
    deployment_slot: u64,
    code_commitment: [u8; IDENTITY_BYTES],
    upgrade_authority: Option<[u8; IDENTITY_BYTES]>,
}

impl DeploymentObservationV2 {
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
        code_commitment: [u8; IDENTITY_BYTES],
        upgrade_authority: Option<[u8; IDENTITY_BYTES]>,
    ) -> Result<Self> {
        for identity in [
            program,
            program_owner,
            programdata,
            programdata_owner,
            programdata_link,
            loader_program,
            code_commitment,
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
            code_commitment,
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
