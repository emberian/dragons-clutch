//! Checked release-set activation and the derived Registry-owned cache.

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_ROLE_COUNT_V1, EXECUTION_ROLE_ORDER_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1,
};

use crate::{
    ARTIFACT_RELEASE_BYTES_V1, ArtifactReleaseV1, DeploymentObservationV1, Error, IDENTITY_BYTES,
    Result, copy_infallible, put_u16, read_array, read_u16, require_zero, subslice,
};

/// Bytes in one activated role projection.
pub const ACTIVATED_ROLE_BYTES_V1: usize = IDENTITY_BYTES + ARTIFACT_RELEASE_BYTES_V1;
/// First PDA seed for the sole Registry-owned activation cache.
///
/// The adapter must derive the cache under the Registry program
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

/// Offset of the cache's own canonical PDA bump.
///
/// The cache address is `[ACTIVATION_PDA_DOMAIN_V1, release_set_id]` under the
/// Registry, and SIX separate searches per top-level Direct transaction land on
/// it -- one in Trading, two inside the Registry's own `Reauthenticate` CPIs,
/// one from Claims and two from Custody. Every one of them already holds and
/// decodes this account, so the cheapest carrier for the bump is the account
/// itself: a reader reproduces the address with `create_program_address` and a
/// wrong byte reproduces a different address and refuses.
///
/// It is the first of what were four canonical reserved bytes, so the width and
/// every offset after it are unchanged.
///
/// **Zero means "written before this byte existed", not "invalid".**
/// `find_program_address` walks 255 down to 1 and never returns 0, so a zero
/// here is an older cache body, and a reader that meets one falls back to the
/// search it used to do unconditionally. That is what makes this deployable
/// without a migration, and it is the same rule
/// `CAPABILITY_EXECUTION_SELECTION_RECORD_BUMPS_OFFSET` already follows in
/// `dclutch-release-set-contract`.
pub const ACTIVATION_CACHE_BUMP_OFFSET_V1: usize = 12;

const SCHEMA_OFFSET: usize = 8;
const PROFILE_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 13;
const RESERVED_BYTES: usize = 3;
const RELEASE_SET_ID_OFFSET: usize = 16;
const ROLES_OFFSET: usize = 48;

const ALL_ROLES: [ExecutionRoleV1; EXECUTION_ROLE_COUNT_V1] = EXECUTION_ROLE_ORDER_V1;

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

    const fn input(&self, role: ExecutionRoleV1) -> &ArtifactActivationInputV1 {
        match role {
            ExecutionRoleV1::Core => &self.core,
            ExecutionRoleV1::Claims => &self.claims,
            ExecutionRoleV1::Trading => &self.trading,
            ExecutionRoleV1::Resolution => &self.resolution,
            ExecutionRoleV1::Custody => &self.custody,
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

/// Borrowed hostile-validated view of one Registry-owned activation cache.
///
/// This view avoids copying all five artifact releases into one SBF stack
/// frame. It validates the same projection and decodes only the role a caller
/// actually consumes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatedExecutionReleaseSetViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> ActivatedExecutionReleaseSetViewV1<'a> {
    /// Hostile-decode one exact complete activation cache without copying it.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        validate_activation_header(bytes)?;
        let value = Self { bytes };
        value.validate_projection()?;
        Ok(value)
    }

    /// The cache address's own canonical PDA bump, if this body carries one.
    ///
    /// `None` for a cache written before the bump was persisted; see
    /// [`ACTIVATION_CACHE_BUMP_OFFSET_V1`] for why that is a fallback and not a
    /// refusal.
    pub fn cache_bump(self) -> Result<Option<u8>> {
        let bump = *self
            .bytes
            .get(ACTIVATION_CACHE_BUMP_OFFSET_V1)
            .ok_or(Error::InvalidLength)?;
        Ok((bump != 0).then_some(bump))
    }

    /// Return the exact activated release-set content identity.
    pub fn execution_release_set_id(self) -> Result<ContentId> {
        ContentId::new(read_array(self.bytes, RELEASE_SET_ID_OFFSET)?)
            .map_err(|_| Error::ZeroIdentity)
    }

    /// Decode one activated role from the borrowed cache.
    pub fn role(self, role: ExecutionRoleV1) -> Result<ActivatedRoleV1> {
        decode_role(self.bytes, role)
    }

    /// Reconstruct the exact release-set projection cached by this state.
    pub fn release_set_projection(self) -> Result<ExecutionReleaseSetV1> {
        ExecutionReleaseSetV1::new(
            projection_binding(self.role(ExecutionRoleV1::Core)?),
            projection_binding(self.role(ExecutionRoleV1::Claims)?),
            projection_binding(self.role(ExecutionRoleV1::Trading)?),
            projection_binding(self.role(ExecutionRoleV1::Resolution)?),
            projection_binding(self.role(ExecutionRoleV1::Custody)?),
        )
        .map_err(Error::from)
    }

    fn validate_projection(self) -> Result<()> {
        self.release_set_projection()?;
        for (left_index, left) in ALL_ROLES.into_iter().enumerate() {
            for right in ALL_ROLES.into_iter().skip(left_index + 1) {
                let left_role = self.role(left)?;
                let right_role = self.role(right)?;
                let same_pair = projection_binding(left_role) == projection_binding(right_role);
                if same_pair && left_role != right_role {
                    return Err(Error::AliasedRoleActivationMismatch);
                }
            }
        }
        Ok(())
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
        let view = ActivatedExecutionReleaseSetViewV1::decode(bytes)?;
        let value = Self {
            execution_release_set_id: view.execution_release_set_id()?,
            core: view.role(ExecutionRoleV1::Core)?,
            claims: view.role(ExecutionRoleV1::Claims)?,
            trading: view.role(ExecutionRoleV1::Trading)?,
            resolution: view.role(ExecutionRoleV1::Resolution)?,
            custody: view.role(ExecutionRoleV1::Custody)?,
        };
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

/// How much of one observed activation cache has been admitted so far.
///
/// Activation admits one role per transaction, so between transactions the
/// Registry-owned cache legitimately holds a strict subset of its five roles.
/// A partially written cache cannot [`ActivatedExecutionReleaseSetViewV1::decode`],
/// so it is inert for every reader; this value exists so an *operator* walking
/// the release set up can tell "not yet admitted" apart from "admitted
/// something else", which is a refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationCacheProgressV1 {
    written: [bool; EXECUTION_ROLE_COUNT_V1],
}

impl ActivationCacheProgressV1 {
    /// Whether this exact role has already been admitted.
    pub fn is_written(self, role: ExecutionRoleV1) -> bool {
        self.written
            .get(role.role_index())
            .copied()
            .unwrap_or(false)
    }

    /// Number of roles already admitted, from zero to five.
    pub fn written_count(self) -> usize {
        self.written.iter().filter(|written| **written).count()
    }

    /// Whether every role has been admitted and the cache is now readable.
    pub fn is_complete(self) -> bool {
        self.written_count() == ALL_ROLES.len()
    }
}

/// Compare one observed activation cache against the complete expected cache.
///
/// Returns which roles have been admitted so far. Refuses when the header does
/// not match, or when any *already written* role slot is not byte-identical to
/// the expected slot — an unwritten slot is exactly all zero and anything else
/// is a different release set masquerading as progress. This never admits
/// anything: it reports what a Registry-owned account already contains.
pub fn activation_cache_progress_v1(
    observed: &[u8],
    expected: ActivatedExecutionReleaseSetV1,
) -> Result<ActivationCacheProgressV1> {
    if observed.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    validate_activation_header(observed)?;
    let complete = expected.to_bytes();
    if subslice(observed, 0, ROLES_OFFSET)? != subslice(&complete, 0, ROLES_OFFSET)? {
        return Err(Error::ReleaseSetSelectionMismatch);
    }
    let mut written = [false; 5];
    for role in ALL_ROLES {
        let offset = role_offset(role);
        let observed_role = subslice(observed, offset, ACTIVATED_ROLE_BYTES_V1)?;
        if observed_role.iter().all(|byte| *byte == 0) {
            continue;
        }
        if observed_role != subslice(&complete, offset, ACTIVATED_ROLE_BYTES_V1)? {
            return Err(Error::AliasedRoleActivationMismatch);
        }
        if let Some(slot) = written.get_mut(role.role_index()) {
            *slot = true;
        }
    }
    Ok(ActivationCacheProgressV1 { written })
}

/// Initialize one transaction-local activation-cache buffer.
///
/// The buffer must be the exact zero-filled account allocation. Call
/// [`activate_execution_role_into_v1`] once for every role, in canonical role
/// order, and finish with [`ActivatedExecutionReleaseSetViewV1::decode`]. A
/// composing SBF adapter may write directly into a newly created PDA because a
/// later refusal rolls the entire transaction back. Registry identity is an
/// account-ownership boundary, not a Core-selection input; the finalized
/// release set binds Core when that role is activated.
pub fn initialize_activation_cache_v1(
    output: &mut [u8],
    finalized_release_set_id: ContentId,
) -> Result<()> {
    if output.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    if output.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    copy_infallible(output, 0, &ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1);
    put_u16(
        output,
        SCHEMA_OFFSET,
        ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1,
    );
    put_u16(
        output,
        PROFILE_OFFSET,
        ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1,
    );
    copy_infallible(
        output,
        RELEASE_SET_ID_OFFSET,
        finalized_release_set_id.as_bytes(),
    );
    Ok(())
}

/// Record the cache address's own canonical PDA bump into an initialized buffer.
///
/// Deliberately separate from [`initialize_activation_cache_v1`] rather than a
/// parameter of it. The bump is an OPTIMIZATION carrier, not a fact the cache
/// needs to be valid, and every reader falls back to the search when the byte
/// is zero. Keeping it separate means the one writer that has a bump to record
/// -- the Registry, which had to derive it to sign the account into existence
/// -- records it, and the couple of dozen harnesses that stage a cache by hand
/// stay exactly as they are and simply do not get the saving.
///
/// Refuses zero, because a caller with no bump has not derived the address it
/// is writing, and refuses a buffer that is not a canonical cache.
pub fn put_activation_cache_bump_v1(output: &mut [u8], cache_bump: u8) -> Result<()> {
    validate_activation_header(output)?;
    if cache_bump == 0 {
        return Err(Error::NonCanonicalReservedBytes);
    }
    *output
        .get_mut(ACTIVATION_CACHE_BUMP_OFFSET_V1)
        .ok_or(Error::InvalidLength)? = cache_bump;
    Ok(())
}

/// Authenticate and write one exact role into an initialized cache buffer.
///
/// A slot may be empty or already contain the exact same activated role.
/// Conflicting rewrites and conflicting bytes for any aliased role refuse.
pub fn activate_execution_role_into_v1(
    output: &mut [u8],
    finalized_release_set_id: ContentId,
    release_set: &ExecutionReleaseSetV1,
    role: ExecutionRoleV1,
    input: &ArtifactActivationInputV1,
) -> Result<()> {
    validate_activation_header(output)?;
    if read_array::<IDENTITY_BYTES>(output, RELEASE_SET_ID_OFFSET)?
        != finalized_release_set_id.to_bytes()
    {
        return Err(Error::ReleaseSetSelectionMismatch);
    }
    let expected = release_set.binding(role);
    authenticate_role(expected, input)?;
    let activated = activated(input);
    let mut encoded_role = [0_u8; ACTIVATED_ROLE_BYTES_V1];
    copy_infallible(
        &mut encoded_role,
        0,
        activated.artifact_release_id.as_bytes(),
    );
    copy_infallible(
        &mut encoded_role,
        IDENTITY_BYTES,
        &activated.release.to_bytes(),
    );
    for other in ALL_ROLES {
        let other_offset = role_offset(other);
        let other_bytes = subslice(output, other_offset, ACTIVATED_ROLE_BYTES_V1)?;
        let initialized = other_bytes.iter().any(|byte| *byte != 0);
        if other == role {
            if initialized && other_bytes != encoded_role {
                return Err(Error::AliasedRoleActivationMismatch);
            }
        } else if expected == release_set.binding(other)
            && initialized
            && other_bytes != encoded_role
        {
            return Err(Error::AliasedRoleActivationMismatch);
        }
    }
    copy_infallible(output, role_offset(role), &encoded_role);
    Ok(())
}

/// Check all finalized releases and current Loader observations, then produce
/// the sole Registry-owned activation cache.
///
/// `finalized_release_set_id` must be the digest of the exact finalized
/// `release_set` bytes, established by the composing record adapter. The Core
/// binding is authenticated exactly like every other role and need not name
/// the Registry program that owns the resulting cache.
pub fn activate_execution_release_set_v1(
    finalized_release_set_id: ContentId,
    release_set: &ExecutionReleaseSetV1,
    inputs: &ExecutionReleaseActivationInputsV1,
) -> Result<ActivatedExecutionReleaseSetV1> {
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
        core: activated(&inputs.core),
        claims: activated(&inputs.claims),
        trading: activated(&inputs.trading),
        resolution: activated(&inputs.resolution),
        custody: activated(&inputs.custody),
    };
    value.validate_projection()?;
    Ok(value)
}

fn authenticate_role(
    expected: ExecutionRoleBindingV1,
    input: &ArtifactActivationInputV1,
) -> Result<()> {
    if input.finalized_artifact_release_id != expected.artifact_release() {
        return Err(Error::RoleArtifactReleaseMismatch);
    }
    if input.release.program() != expected.program() {
        return Err(Error::RoleProgramMismatch);
    }
    input.release.authenticate_deployment(input.deployment)
}

const fn activated(input: &ArtifactActivationInputV1) -> ActivatedRoleV1 {
    ActivatedRoleV1 {
        artifact_release_id: input.finalized_artifact_release_id,
        release: input.release,
    }
}

fn projection_binding(activated: ActivatedRoleV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(activated.release.program(), activated.artifact_release_id)
}

fn role_offset(role: ExecutionRoleV1) -> usize {
    ROLES_OFFSET + role.role_index() * ACTIVATED_ROLE_BYTES_V1
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

fn validate_activation_header(bytes: &[u8]) -> Result<()> {
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
    require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)
}
