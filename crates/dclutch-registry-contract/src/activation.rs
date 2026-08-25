//! Checked release-set activation and the derived Registry-owned cache.

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};

use crate::{
    ARTIFACT_RELEASE_BYTES_V1, ArtifactReleaseV1, DeploymentObservationV1, Error, IDENTITY_BYTES,
    Result, copy_infallible, put_u16, read_array, read_u16, require_zero, subslice,
};

/// Bytes in one activated role projection.
pub const ACTIVATED_ROLE_BYTES_V1: usize = IDENTITY_BYTES + ARTIFACT_RELEASE_BYTES_V1;
/// First PDA seed for the sole Registry-owned activation cache.
///
/// The adapter must derive the cache under the selected Registry/Core program
/// with exactly `[ACTIVATION_PDA_DOMAIN_V1, execution_release_set_id]`, in that
/// order, and no caller-selected seed.
pub const ACTIVATION_PDA_DOMAIN_V1: &[u8; 29] = b"dclutch:release-activation:v1";
/// Bytes in the complete Registry-owned activation cache.
pub const ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1: usize = 48 + 5 * ACTIVATED_ROLE_BYTES_V1;
/// Canonical activation-cache magic.
pub const ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1: [u8; 8] = *b"DCLTACT1";
/// Implemented activation-cache schema.
pub const ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1: u16 = 1;
/// Implemented activation-cache fixed-layout profile.
pub const ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1: u16 = 1;
/// Schema/validator identity for activation-cache accounts.
///
/// This is SHA-256 of `dclutch/schema/activated-execution-release-set-v1`.
pub const ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_ID_V1: [u8; IDENTITY_BYTES] = [
    0xb5, 0xec, 0x61, 0x33, 0x76, 0xbf, 0xd1, 0x6b, 0x8f, 0x9c, 0xd3, 0x4d, 0x67, 0xb1, 0x69, 0x35,
    0x4f, 0x72, 0x85, 0xdd, 0xc2, 0x0e, 0xec, 0xcc, 0xdf, 0x24, 0x51, 0x09, 0xe5, 0x7b, 0xbf, 0x66,
];

const SCHEMA_OFFSET: usize = 8;
const PROFILE_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 12;
const RESERVED_BYTES: usize = 4;
const RELEASE_SET_ID_OFFSET: usize = 16;
const ROLES_OFFSET: usize = 48;

const ALL_ROLES: [ExecutionRoleV1; 5] = [
    ExecutionRoleV1::Core,
    ExecutionRoleV1::Claims,
    ExecutionRoleV1::Trading,
    ExecutionRoleV1::Resolution,
    ExecutionRoleV1::Custody,
];

/// Finalized artifact release plus its current deployment observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactActivationInputV1 {
    finalized_artifact_release_id: ArtifactReleaseIdV1,
    release: ArtifactReleaseV1,
    deployment: DeploymentObservationV1,
}

impl ArtifactActivationInputV1 {
    /// Construct one typed activation input.
    pub const fn new(
        finalized_artifact_release_id: ArtifactReleaseIdV1,
        release: ArtifactReleaseV1,
        deployment: DeploymentObservationV1,
    ) -> Self {
        Self {
            finalized_artifact_release_id,
            release,
            deployment,
        }
    }
}

/// Complete exact role-ordered activation input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionReleaseActivationInputsV1 {
    core: ArtifactActivationInputV1,
    claims: ArtifactActivationInputV1,
    trading: ArtifactActivationInputV1,
    resolution: ArtifactActivationInputV1,
    custody: ArtifactActivationInputV1,
}

impl ExecutionReleaseActivationInputsV1 {
    /// Construct one named, role-complete activation input.
    pub const fn new(
        core: ArtifactActivationInputV1,
        claims: ArtifactActivationInputV1,
        trading: ArtifactActivationInputV1,
        resolution: ArtifactActivationInputV1,
        custody: ArtifactActivationInputV1,
    ) -> Self {
        Self {
            core,
            claims,
            trading,
            resolution,
            custody,
        }
    }

    const fn input(self, role: ExecutionRoleV1) -> ArtifactActivationInputV1 {
        match role {
            ExecutionRoleV1::Core => self.core,
            ExecutionRoleV1::Claims => self.claims,
            ExecutionRoleV1::Trading => self.trading,
            ExecutionRoleV1::Resolution => self.resolution,
            ExecutionRoleV1::Custody => self.custody,
        }
    }
}

/// One activated role and its exact canonical artifact release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatedRoleV1 {
    artifact_release_id: ArtifactReleaseIdV1,
    release: ArtifactReleaseV1,
}

impl ActivatedRoleV1 {
    /// Return the finalized artifact-release content identity.
    pub const fn artifact_release_id(self) -> ArtifactReleaseIdV1 {
        self.artifact_release_id
    }

    /// Return the exact activated artifact release.
    pub const fn release(self) -> ArtifactReleaseV1 {
        self.release
    }

    /// Reauthenticate the current deployment before lending role authority.
    pub fn authenticate_current_deployment(
        self,
        observation: DeploymentObservationV1,
    ) -> Result<()> {
        self.release.authenticate_deployment(observation)
    }
}

/// Registry-owned derived cache for one fully activated release set.
///
/// Every embedded release is an exact byte-for-byte projection of its sole
/// finalized artifact-release authority.  The account adapter must derive this
/// state's PDA from `execution_release_set_id` and require Registry ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatedExecutionReleaseSetV1 {
    execution_release_set_id: ContentId,
    core: ActivatedRoleV1,
    claims: ActivatedRoleV1,
    trading: ActivatedRoleV1,
    resolution: ActivatedRoleV1,
    custody: ActivatedRoleV1,
}

impl ActivatedExecutionReleaseSetV1 {
    /// Hostile-decode one exact Registry-owned activation cache.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1.len())
            != Some(ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1.as_slice())
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, PROFILE_OFFSET)? != ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        let value = Self {
            execution_release_set_id: ContentId::new(read_array(bytes, RELEASE_SET_ID_OFFSET)?)
                .map_err(|_| Error::ZeroIdentity)?,
            core: decode_role(bytes, ExecutionRoleV1::Core)?,
            claims: decode_role(bytes, ExecutionRoleV1::Claims)?,
            trading: decode_role(bytes, ExecutionRoleV1::Trading)?,
            resolution: decode_role(bytes, ExecutionRoleV1::Resolution)?,
            custody: decode_role(bytes, ExecutionRoleV1::Custody)?,
        };
        value.validate_projection()?;
        Ok(value)
    }

    /// Encode the exact Registry-owned activation-cache state.
    pub fn to_bytes(self) -> [u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1] {
        let mut output = [0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        copy_infallible(&mut output, 0, &ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1);
        put_u16(
            &mut output,
            SCHEMA_OFFSET,
            ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1,
        );
        put_u16(
            &mut output,
            PROFILE_OFFSET,
            ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1,
        );
        copy_infallible(
            &mut output,
            RELEASE_SET_ID_OFFSET,
            self.execution_release_set_id.as_bytes(),
        );
        for role in ALL_ROLES {
            let activated = self.role(role);
            let offset = role_offset(role);
            copy_infallible(
                &mut output,
                offset,
                activated.artifact_release_id.as_bytes(),
            );
            copy_infallible(
                &mut output,
                offset + IDENTITY_BYTES,
                &activated.release.to_bytes(),
            );
        }
        output
    }

    /// Return the exact activated release-set content identity.
    pub const fn execution_release_set_id(self) -> ContentId {
        self.execution_release_set_id
    }

    /// Return one activated semantic role.
    pub const fn role(self, role: ExecutionRoleV1) -> ActivatedRoleV1 {
        match role {
            ExecutionRoleV1::Core => self.core,
            ExecutionRoleV1::Claims => self.claims,
            ExecutionRoleV1::Trading => self.trading,
            ExecutionRoleV1::Resolution => self.resolution,
            ExecutionRoleV1::Custody => self.custody,
        }
    }

    /// Reconstruct the exact release-set projection cached by this state.
    pub fn release_set_projection(self) -> Result<ExecutionReleaseSetV1> {
        ExecutionReleaseSetV1::new(
            projection_binding(self.core),
            projection_binding(self.claims),
            projection_binding(self.trading),
            projection_binding(self.resolution),
            projection_binding(self.custody),
        )
        .map_err(Error::from)
    }

    fn validate_projection(self) -> Result<()> {
        self.release_set_projection()?;
        for (left_index, left) in ALL_ROLES.into_iter().enumerate() {
            for right in ALL_ROLES.into_iter().skip(left_index + 1) {
                let left_role = self.role(left);
                let right_role = self.role(right);
                let same_pair = projection_binding(left_role) == projection_binding(right_role);
                if same_pair && left_role != right_role {
                    return Err(Error::AliasedRoleActivationMismatch);
                }
            }
        }
        Ok(())
    }
}

/// Check all finalized releases and current Loader observations, then produce
/// the sole Registry-owned activation cache.
///
/// `finalized_release_set_id` must be the digest of the exact finalized
/// `release_set` bytes, established by the composing record adapter.
pub fn activate_execution_release_set_v1(
    current_core_program: ProgramIdentityV1,
    finalized_release_set_id: ContentId,
    release_set: ExecutionReleaseSetV1,
    inputs: ExecutionReleaseActivationInputsV1,
) -> Result<ActivatedExecutionReleaseSetV1> {
    if release_set.binding(ExecutionRoleV1::Core).program() != current_core_program {
        return Err(Error::CoreProgramMismatch);
    }
    for (left_index, left) in ALL_ROLES.into_iter().enumerate() {
        let expected = release_set.binding(left);
        let input = inputs.input(left);
        authenticate_role(expected, input)?;
        for right in ALL_ROLES.into_iter().skip(left_index + 1) {
            if expected == release_set.binding(right) && input != inputs.input(right) {
                return Err(Error::AliasedRoleActivationMismatch);
            }
        }
    }
    let value = ActivatedExecutionReleaseSetV1 {
        execution_release_set_id: finalized_release_set_id,
        core: activated(inputs.core),
        claims: activated(inputs.claims),
        trading: activated(inputs.trading),
        resolution: activated(inputs.resolution),
        custody: activated(inputs.custody),
    };
    value.validate_projection()?;
    Ok(value)
}

fn authenticate_role(
    expected: ExecutionRoleBindingV1,
    input: ArtifactActivationInputV1,
) -> Result<()> {
    if input.finalized_artifact_release_id != expected.artifact_release() {
        return Err(Error::RoleArtifactReleaseMismatch);
    }
    if input.release.program() != expected.program() {
        return Err(Error::RoleProgramMismatch);
    }
    input.release.authenticate_deployment(input.deployment)
}

const fn activated(input: ArtifactActivationInputV1) -> ActivatedRoleV1 {
    ActivatedRoleV1 {
        artifact_release_id: input.finalized_artifact_release_id,
        release: input.release,
    }
}

fn projection_binding(activated: ActivatedRoleV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(activated.release.program(), activated.artifact_release_id)
}

fn role_offset(role: ExecutionRoleV1) -> usize {
    let index = match role {
        ExecutionRoleV1::Core => 0,
        ExecutionRoleV1::Claims => 1,
        ExecutionRoleV1::Trading => 2,
        ExecutionRoleV1::Resolution => 3,
        ExecutionRoleV1::Custody => 4,
    };
    ROLES_OFFSET + index * ACTIVATED_ROLE_BYTES_V1
}

fn decode_role(bytes: &[u8], role: ExecutionRoleV1) -> Result<ActivatedRoleV1> {
    let offset = role_offset(role);
    Ok(ActivatedRoleV1 {
        artifact_release_id: ArtifactReleaseIdV1::decode(subslice(bytes, offset, IDENTITY_BYTES)?)?,
        release: ArtifactReleaseV1::decode(subslice(
            bytes,
            offset + IDENTITY_BYTES,
            ARTIFACT_RELEASE_BYTES_V1,
        )?)?,
    })
}
