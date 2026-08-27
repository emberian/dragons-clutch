//! Canonical family-neutral batched Registry role authentication wires.
//!
//! One request names the exact activation-cache digest, release set, and a
//! strictly increasing subset of the five execution roles. One fixed-width
//! receipt binds that request to the Registry program, cache PDA, and every
//! exact Program/ProgramData current deployment observation. Inactive receipt
//! slots are canonical zero. Hashing and return-data producer checks remain
//! adapter obligations.

use core::convert::TryInto;

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_ROLE_COUNT_V1, ExecutionRoleV1, ProgramIdentityV1,
};

/// Exact batched Registry instruction width.
pub const ROLE_BATCH_REQUEST_BYTES_V2: usize = 96;
/// Exact fixed five-slot batch receipt width.
pub const ROLE_BATCH_RECEIPT_BYTES_V2: usize = 896;
/// Exact width of one active role deployment observation.
pub const ROLE_BATCH_OBSERVATION_BYTES_V2: usize = 144;
/// Canonical batch request magic.
pub const ROLE_BATCH_REQUEST_MAGIC_V2: [u8; 8] = *b"DCLTRGB2";
/// Canonical batch receipt magic.
pub const ROLE_BATCH_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLTRGR2";
/// Implemented batch wire schema.
pub const ROLE_BATCH_SCHEMA_V2: u16 = 2;

const REQUEST_COUNT_OFFSET: usize = 10;
const REQUEST_MASK_OFFSET: usize = 11;
const REQUEST_RESERVED_OFFSET: usize = 12;
const REQUEST_RELEASE_SET_OFFSET: usize = 16;
const REQUEST_CACHE_DIGEST_OFFSET: usize = 48;
const REQUEST_ROLES_OFFSET: usize = 80;
const REQUEST_TAIL_RESERVED_OFFSET: usize = 85;

const RECEIPT_COUNT_OFFSET: usize = 10;
const RECEIPT_MASK_OFFSET: usize = 11;
const RECEIPT_RESERVED_OFFSET: usize = 12;
const RECEIPT_REGISTRY_OFFSET: usize = 16;
const RECEIPT_CACHE_OFFSET: usize = 48;
const RECEIPT_CACHE_DIGEST_OFFSET: usize = 80;
const RECEIPT_RELEASE_SET_OFFSET: usize = 112;
const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 144;
const RECEIPT_OBSERVATIONS_OFFSET: usize = 176;

const OBSERVATION_ROLE_OFFSET: usize = 0;
const OBSERVATION_RESERVED_OFFSET: usize = 1;
const OBSERVATION_PROGRAM_OFFSET: usize = 8;
const OBSERVATION_PROGRAMDATA_OFFSET: usize = 40;
const OBSERVATION_ARTIFACT_OFFSET: usize = 72;
const OBSERVATION_SEMANTIC_OFFSET: usize = 104;
const OBSERVATION_DEPLOYMENT_SLOT_OFFSET: usize = 136;

/// Stable refusal from a hostile batched Registry wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchErrorV2 {
    /// Input or output did not have its one exact width.
    InvalidLength,
    /// Magic or schema selected another wire family.
    UnsupportedSchema,
    /// Reserved or inactive bytes were nonzero.
    NonCanonicalReserved,
    /// Requested role count was empty or exceeded the fixed release-set profile.
    InvalidRoleCount,
    /// A role tag was outside the release-set profile.
    UnknownRole,
    /// Requested roles were duplicated or not in canonical order.
    NonCanonicalRoleOrder,
    /// The encoded mask did not exactly equal the ordered role list.
    RoleMaskMismatch,
    /// A persisted identity used the all-zero sentinel.
    ZeroIdentity,
    /// A typed identity refused hostile bytes.
    Identity,
    /// A typed continuation profile did not match its exact roles or coordinates.
    ContinuationProfileMismatch,
}

/// Result alias for batched Registry wires.
pub type Result<T> = core::result::Result<T, BatchErrorV2>;

/// One exact canonical batch request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleBatchRequestV2 {
    release_set_id: ContentId,
    activation_cache_digest: ContentId,
    role_count: u8,
    role_mask: u8,
    roles: [u8; EXECUTION_ROLE_COUNT_V1],
}

impl RoleBatchRequestV2 {
    /// Construct one nonempty strictly role-ordered request.
    pub fn new(
        release_set_id: ContentId,
        activation_cache_digest: ContentId,
        roles: &[ExecutionRoleV1],
    ) -> Result<Self> {
        let role_count = u8::try_from(roles.len()).map_err(|_| BatchErrorV2::InvalidRoleCount)?;
        if roles.is_empty() || roles.len() > EXECUTION_ROLE_COUNT_V1 {
            return Err(BatchErrorV2::InvalidRoleCount);
        }
        let mut encoded_roles = [0_u8; EXECUTION_ROLE_COUNT_V1];
        let mut role_mask = 0_u8;
        let mut prior = None;
        for (index, role) in roles.iter().copied().enumerate() {
            let tag = role_tag(role);
            if prior.is_some_and(|value| tag <= value) {
                return Err(BatchErrorV2::NonCanonicalRoleOrder);
            }
            *encoded_roles
                .get_mut(index)
                .ok_or(BatchErrorV2::InvalidRoleCount)? = tag;
            role_mask |= role_bit(tag)?;
            prior = Some(tag);
        }
        Ok(Self {
            release_set_id,
            activation_cache_digest,
            role_count,
            role_mask,
            roles: encoded_roles,
        })
    }

    /// Hostile-decode the one exact canonical request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            &ROLE_BATCH_REQUEST_MAGIC_V2,
            ROLE_BATCH_REQUEST_BYTES_V2,
        )?;
        require_zero(bytes, REQUEST_RESERVED_OFFSET, 4)?;
        require_zero(bytes, REQUEST_TAIL_RESERVED_OFFSET, 11)?;
        let count = usize::from(byte(bytes, REQUEST_COUNT_OFFSET)?);
        if count == 0 || count > EXECUTION_ROLE_COUNT_V1 {
            return Err(BatchErrorV2::InvalidRoleCount);
        }
        let role_bytes = slice(bytes, REQUEST_ROLES_OFFSET, EXECUTION_ROLE_COUNT_V1)?;
        require_zero(role_bytes, count, EXECUTION_ROLE_COUNT_V1 - count)?;
        let mut roles = [ExecutionRoleV1::Core; EXECUTION_ROLE_COUNT_V1];
        for (index, tag) in role_bytes.iter().copied().take(count).enumerate() {
            *roles.get_mut(index).ok_or(BatchErrorV2::InvalidRoleCount)? = decode_role(tag)?;
        }
        let value = Self::new(
            content_id(bytes, REQUEST_RELEASE_SET_OFFSET)?,
            content_id(bytes, REQUEST_CACHE_DIGEST_OFFSET)?,
            roles.get(..count).ok_or(BatchErrorV2::InvalidRoleCount)?,
        )?;
        if value.role_mask != byte(bytes, REQUEST_MASK_OFFSET)? {
            return Err(BatchErrorV2::RoleMaskMismatch);
        }
        Ok(value)
    }

    /// Encode the one exact canonical request.
    pub fn to_bytes(self) -> [u8; ROLE_BATCH_REQUEST_BYTES_V2] {
        let mut output = [0_u8; ROLE_BATCH_REQUEST_BYTES_V2];
        put(&mut output, 0, &ROLE_BATCH_REQUEST_MAGIC_V2);
        put(&mut output, 8, &ROLE_BATCH_SCHEMA_V2.to_le_bytes());
        set(&mut output, REQUEST_COUNT_OFFSET, self.role_count);
        set(&mut output, REQUEST_MASK_OFFSET, self.role_mask);
        put(
            &mut output,
            REQUEST_RELEASE_SET_OFFSET,
            self.release_set_id.as_bytes(),
        );
        put(
            &mut output,
            REQUEST_CACHE_DIGEST_OFFSET,
            self.activation_cache_digest.as_bytes(),
        );
        put(&mut output, REQUEST_ROLES_OFFSET, &self.roles);
        output
    }

    /// Exact selected release-set identity.
    pub const fn release_set_id(self) -> ContentId {
        self.release_set_id
    }

    /// Expected SHA-256 digest of the complete activation-cache bytes.
    pub const fn activation_cache_digest(self) -> ContentId {
        self.activation_cache_digest
    }

    /// Nonempty requested role count.
    pub const fn role_count(self) -> u8 {
        self.role_count
    }

    /// Exact bitmask implied by the canonical ordered list.
    pub const fn role_mask(self) -> u8 {
        self.role_mask
    }

    /// Read one requested role.
    pub fn role(self, index: usize) -> Option<ExecutionRoleV1> {
        if index >= usize::from(self.role_count) {
            return None;
        }
        self.roles
            .get(index)
            .copied()
            .and_then(|tag| decode_role(tag).ok())
    }
}

/// One exact current deployment observation placed in a batch receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleDeploymentObservationV2 {
    role: ExecutionRoleV1,
    program: ProgramIdentityV1,
    programdata: [u8; 32],
    artifact_release_id: ArtifactReleaseIdV1,
    semantic_release_id: ContentId,
    deployment_slot: u64,
}

impl RoleDeploymentObservationV2 {
    /// Construct one already-authenticated deployment observation.
    pub fn new(
        role: ExecutionRoleV1,
        program: ProgramIdentityV1,
        programdata: [u8; 32],
        artifact_release_id: ArtifactReleaseIdV1,
        semantic_release_id: ContentId,
        deployment_slot: u64,
    ) -> Result<Self> {
        if programdata.iter().all(|byte| *byte == 0) {
            return Err(BatchErrorV2::ZeroIdentity);
        }
        Ok(Self {
            role,
            program,
            programdata,
            artifact_release_id,
            semantic_release_id,
            deployment_slot,
        })
    }

    /// Authenticated role.
    pub const fn role(self) -> ExecutionRoleV1 {
        self.role
    }

    /// Exact executable Program identity.
    pub const fn program(self) -> ProgramIdentityV1 {
        self.program
    }

    /// Exact current ProgramData identity.
    pub const fn programdata(self) -> [u8; 32] {
        self.programdata
    }

    /// Exact finalized artifact-release identity.
    pub const fn artifact_release_id(self) -> ArtifactReleaseIdV1 {
        self.artifact_release_id
    }

    /// Exact semantic release implemented by the artifact.
    pub const fn semantic_release_id(self) -> ContentId {
        self.semantic_release_id
    }

    /// Exact current ProgramData deployment slot.
    pub const fn deployment_slot(self) -> u64 {
        self.deployment_slot
    }
}

/// Inputs for one fixed-width authenticated role-set receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleBatchReceiptInputV2<'a> {
    /// Registry program that produced the immediate return data.
    pub registry_program: ProgramIdentityV1,
    /// Canonical activation-cache PDA.
    pub activation_cache: [u8; 32],
    /// SHA-256 digest of the exact authenticated cache bytes.
    pub activation_cache_digest: ContentId,
    /// Exact selected execution release set.
    pub release_set_id: ContentId,
    /// SHA-256 digest of the exact batch request bytes.
    pub request_digest: ContentId,
    /// Strictly role-ordered current deployment observations.
    pub observations: &'a [RoleDeploymentObservationV2],
}

/// Borrowed hostile-validated view of one fixed role-set receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRoleBatchReceiptV2<'a> {
    bytes: &'a [u8],
    role_count: u8,
    role_mask: u8,
    registry_program: ProgramIdentityV1,
    activation_cache: [u8; 32],
    activation_cache_digest: ContentId,
    release_set_id: ContentId,
    request_digest: ContentId,
}

impl<'a> AuthenticatedRoleBatchReceiptV2<'a> {
    /// Hostile-decode and fully validate one exact fixed receipt.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        require_header(
            bytes,
            &ROLE_BATCH_RECEIPT_MAGIC_V2,
            ROLE_BATCH_RECEIPT_BYTES_V2,
        )?;
        require_zero(bytes, RECEIPT_RESERVED_OFFSET, 4)?;
        let role_count = byte(bytes, RECEIPT_COUNT_OFFSET)?;
        let count = usize::from(role_count);
        if count == 0 || count > EXECUTION_ROLE_COUNT_V1 {
            return Err(BatchErrorV2::InvalidRoleCount);
        }
        let value = Self {
            bytes,
            role_count,
            role_mask: byte(bytes, RECEIPT_MASK_OFFSET)?,
            registry_program: program_id(bytes, RECEIPT_REGISTRY_OFFSET)?,
            activation_cache: nonzero_array(bytes, RECEIPT_CACHE_OFFSET)?,
            activation_cache_digest: content_id(bytes, RECEIPT_CACHE_DIGEST_OFFSET)?,
            release_set_id: content_id(bytes, RECEIPT_RELEASE_SET_OFFSET)?,
            request_digest: content_id(bytes, RECEIPT_REQUEST_DIGEST_OFFSET)?,
        };
        let mut mask = 0_u8;
        let mut prior = None;
        for index in 0..count {
            let observation = value
                .observation(index)
                .ok_or(BatchErrorV2::InvalidLength)??;
            let tag = role_tag(observation.role);
            if prior.is_some_and(|previous| tag <= previous) {
                return Err(BatchErrorV2::NonCanonicalRoleOrder);
            }
            mask |= role_bit(tag)?;
            prior = Some(tag);
        }
        let inactive_offset = RECEIPT_OBSERVATIONS_OFFSET
            .checked_add(
                count
                    .checked_mul(ROLE_BATCH_OBSERVATION_BYTES_V2)
                    .ok_or(BatchErrorV2::InvalidLength)?,
            )
            .ok_or(BatchErrorV2::InvalidLength)?;
        require_zero(
            bytes,
            inactive_offset,
            (EXECUTION_ROLE_COUNT_V1 - count)
                .checked_mul(ROLE_BATCH_OBSERVATION_BYTES_V2)
                .ok_or(BatchErrorV2::InvalidLength)?,
        )?;
        if mask != value.role_mask {
            return Err(BatchErrorV2::RoleMaskMismatch);
        }
        Ok(value)
    }

    /// Producing Registry program recorded by the receipt.
    pub const fn registry_program(self) -> ProgramIdentityV1 {
        self.registry_program
    }

    /// Canonical authenticated activation-cache PDA.
    pub const fn activation_cache(self) -> [u8; 32] {
        self.activation_cache
    }

    /// SHA-256 digest of exact activation-cache bytes.
    pub const fn activation_cache_digest(self) -> ContentId {
        self.activation_cache_digest
    }

    /// Exact selected release-set identity.
    pub const fn release_set_id(self) -> ContentId {
        self.release_set_id
    }

    /// SHA-256 digest of the exact request bytes.
    pub const fn request_digest(self) -> ContentId {
        self.request_digest
    }

    /// Number of active ordered observations.
    pub const fn role_count(self) -> u8 {
        self.role_count
    }

    /// Exact mask implied by active observations.
    pub const fn role_mask(self) -> u8 {
        self.role_mask
    }

    /// Decode one active current deployment observation.
    pub fn observation(self, index: usize) -> Option<Result<RoleDeploymentObservationV2>> {
        if index >= usize::from(self.role_count) {
            return None;
        }
        Some(decode_observation(self.bytes, index))
    }
}

/// Encode one fixed-width batch receipt from authenticated observations.
pub fn encode_role_batch_receipt_v2(
    input: RoleBatchReceiptInputV2<'_>,
    output: &mut [u8],
) -> Result<()> {
    if output.len() != ROLE_BATCH_RECEIPT_BYTES_V2 {
        return Err(BatchErrorV2::InvalidLength);
    }
    if input.activation_cache.iter().all(|byte| *byte == 0) {
        return Err(BatchErrorV2::ZeroIdentity);
    }
    if input.observations.is_empty() || input.observations.len() > EXECUTION_ROLE_COUNT_V1 {
        return Err(BatchErrorV2::InvalidRoleCount);
    }
    let mut prior = None;
    let mut mask = 0_u8;
    for observation in input.observations {
        let tag = role_tag(observation.role);
        if prior.is_some_and(|previous| tag <= previous) {
            return Err(BatchErrorV2::NonCanonicalRoleOrder);
        }
        mask |= role_bit(tag)?;
        prior = Some(tag);
    }
    output.fill(0);
    put(output, 0, &ROLE_BATCH_RECEIPT_MAGIC_V2);
    put(output, 8, &ROLE_BATCH_SCHEMA_V2.to_le_bytes());
    set(
        output,
        RECEIPT_COUNT_OFFSET,
        u8::try_from(input.observations.len()).map_err(|_| BatchErrorV2::InvalidRoleCount)?,
    );
    set(output, RECEIPT_MASK_OFFSET, mask);
    put(
        output,
        RECEIPT_REGISTRY_OFFSET,
        input.registry_program.as_bytes(),
    );
    put(output, RECEIPT_CACHE_OFFSET, &input.activation_cache);
    put(
        output,
        RECEIPT_CACHE_DIGEST_OFFSET,
        input.activation_cache_digest.as_bytes(),
    );
    put(
        output,
        RECEIPT_RELEASE_SET_OFFSET,
        input.release_set_id.as_bytes(),
    );
    put(
        output,
        RECEIPT_REQUEST_DIGEST_OFFSET,
        input.request_digest.as_bytes(),
    );
    for (index, observation) in input.observations.iter().copied().enumerate() {
        encode_observation(output, index, observation)?;
    }
    let _ = AuthenticatedRoleBatchReceiptV2::decode(output)?;
    Ok(())
}

fn encode_observation(
    output: &mut [u8],
    index: usize,
    observation: RoleDeploymentObservationV2,
) -> Result<()> {
    let offset = observation_offset(index)?;
    set(
        output,
        offset + OBSERVATION_ROLE_OFFSET,
        role_tag(observation.role),
    );
    put(
        output,
        offset + OBSERVATION_PROGRAM_OFFSET,
        observation.program.as_bytes(),
    );
    put(
        output,
        offset + OBSERVATION_PROGRAMDATA_OFFSET,
        &observation.programdata,
    );
    put(
        output,
        offset + OBSERVATION_ARTIFACT_OFFSET,
        observation.artifact_release_id.as_bytes(),
    );
    put(
        output,
        offset + OBSERVATION_SEMANTIC_OFFSET,
        observation.semantic_release_id.as_bytes(),
    );
    put(
        output,
        offset + OBSERVATION_DEPLOYMENT_SLOT_OFFSET,
        &observation.deployment_slot.to_le_bytes(),
    );
    Ok(())
}

fn decode_observation(bytes: &[u8], index: usize) -> Result<RoleDeploymentObservationV2> {
    let offset = observation_offset(index)?;
    require_zero(bytes, offset + OBSERVATION_RESERVED_OFFSET, 7)?;
    RoleDeploymentObservationV2::new(
        decode_role(byte(bytes, offset + OBSERVATION_ROLE_OFFSET)?)?,
        program_id(bytes, offset + OBSERVATION_PROGRAM_OFFSET)?,
        nonzero_array(bytes, offset + OBSERVATION_PROGRAMDATA_OFFSET)?,
        ArtifactReleaseIdV1::decode(slice(bytes, offset + OBSERVATION_ARTIFACT_OFFSET, 32)?)
            .map_err(|_| BatchErrorV2::Identity)?,
        content_id(bytes, offset + OBSERVATION_SEMANTIC_OFFSET)?,
        read_u64(bytes, offset + OBSERVATION_DEPLOYMENT_SLOT_OFFSET)?,
    )
}

fn observation_offset(index: usize) -> Result<usize> {
    RECEIPT_OBSERVATIONS_OFFSET
        .checked_add(
            index
                .checked_mul(ROLE_BATCH_OBSERVATION_BYTES_V2)
                .ok_or(BatchErrorV2::InvalidLength)?,
        )
        .ok_or(BatchErrorV2::InvalidLength)
}

fn require_header(bytes: &[u8], magic: &[u8; 8], width: usize) -> Result<()> {
    if bytes.len() != width {
        return Err(BatchErrorV2::InvalidLength);
    }
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(BatchErrorV2::UnsupportedSchema);
    }
    if read_u16(bytes, 8)? != ROLE_BATCH_SCHEMA_V2 {
        return Err(BatchErrorV2::UnsupportedSchema);
    }
    Ok(())
}

fn decode_role(tag: u8) -> Result<ExecutionRoleV1> {
    match tag {
        0 => Ok(ExecutionRoleV1::Core),
        1 => Ok(ExecutionRoleV1::Claims),
        2 => Ok(ExecutionRoleV1::Trading),
        3 => Ok(ExecutionRoleV1::Resolution),
        4 => Ok(ExecutionRoleV1::Custody),
        _ => Err(BatchErrorV2::UnknownRole),
    }
}

const fn role_tag(role: ExecutionRoleV1) -> u8 {
    match role {
        ExecutionRoleV1::Core => 0,
        ExecutionRoleV1::Claims => 1,
        ExecutionRoleV1::Trading => 2,
        ExecutionRoleV1::Resolution => 3,
        ExecutionRoleV1::Custody => 4,
    }
}

fn role_bit(tag: u8) -> Result<u8> {
    1_u8.checked_shl(u32::from(tag))
        .ok_or(BatchErrorV2::UnknownRole)
}

fn content_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(nonzero_array(bytes, offset)?).map_err(|_| BatchErrorV2::Identity)
}

fn program_id(bytes: &[u8], offset: usize) -> Result<ProgramIdentityV1> {
    ProgramIdentityV1::new(nonzero_array(bytes, offset)?).map_err(|_| BatchErrorV2::Identity)
}

fn nonzero_array(bytes: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value: [u8; 32] = slice(bytes, offset, 32)?
        .try_into()
        .map_err(|_| BatchErrorV2::InvalidLength)?;
    if value.iter().all(|byte| *byte == 0) {
        return Err(BatchErrorV2::ZeroIdentity);
    }
    Ok(value)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(BatchErrorV2::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| BatchErrorV2::InvalidLength)?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        slice(bytes, offset, 8)?
            .try_into()
            .map_err(|_| BatchErrorV2::InvalidLength)?,
    ))
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(width)
        .ok_or(BatchErrorV2::InvalidLength)?;
    bytes.get(offset..end).ok_or(BatchErrorV2::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().any(|byte| *byte != 0) {
        return Err(BatchErrorV2::NonCanonicalReserved);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    if let Some(end) = offset.checked_add(source.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(source);
    }
}

fn set(output: &mut [u8], offset: usize, value: u8) {
    if let Some(destination) = output.get_mut(offset) {
        *destination = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(fill: u8) -> [u8; 32] {
        [fill; 32]
    }

    fn content(fill: u8) -> ContentId {
        ContentId::new(id(fill)).expect("content")
    }

    fn program(fill: u8) -> ProgramIdentityV1 {
        ProgramIdentityV1::new(id(fill)).expect("program")
    }

    fn artifact(fill: u8) -> ArtifactReleaseIdV1 {
        ArtifactReleaseIdV1::new(id(fill)).expect("artifact")
    }

    fn observation(role: ExecutionRoleV1, fill: u8) -> RoleDeploymentObservationV2 {
        RoleDeploymentObservationV2::new(
            role,
            program(fill),
            id(fill + 20),
            artifact(fill + 40),
            content(fill + 60),
            u64::from(fill) * 10,
        )
        .expect("observation")
    }

    #[test]
    fn request_requires_nonempty_unique_canonical_role_order_and_exact_mask() {
        let roles = [
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Trading,
            ExecutionRoleV1::Custody,
        ];
        let value = RoleBatchRequestV2::new(content(1), content(2), &roles).expect("request");
        let bytes = value.to_bytes();
        assert_eq!(RoleBatchRequestV2::decode(&bytes), Ok(value));
        assert_eq!(value.role_mask(), 0b1_0111);
        assert_eq!(value.role(3), Some(ExecutionRoleV1::Custody));
        assert_eq!(value.role(4), None);
        assert_eq!(
            RoleBatchRequestV2::new(content(1), content(2), &[]),
            Err(BatchErrorV2::InvalidRoleCount)
        );
        assert_eq!(
            RoleBatchRequestV2::new(
                content(1),
                content(2),
                &[ExecutionRoleV1::Core, ExecutionRoleV1::Core],
            ),
            Err(BatchErrorV2::NonCanonicalRoleOrder)
        );
        assert_eq!(
            RoleBatchRequestV2::new(
                content(1),
                content(2),
                &[ExecutionRoleV1::Trading, ExecutionRoleV1::Claims],
            ),
            Err(BatchErrorV2::NonCanonicalRoleOrder)
        );
        let mut bad_mask = bytes;
        *bad_mask.get_mut(REQUEST_MASK_OFFSET).expect("mask byte") ^= 1 << 3;
        assert_eq!(
            RoleBatchRequestV2::decode(&bad_mask),
            Err(BatchErrorV2::RoleMaskMismatch)
        );
        let mut inactive = bytes;
        *inactive
            .get_mut(REQUEST_ROLES_OFFSET + roles.len())
            .expect("inactive role byte") = 4;
        assert_eq!(
            RoleBatchRequestV2::decode(&inactive),
            Err(BatchErrorV2::NonCanonicalReserved)
        );
        assert_eq!(
            RoleBatchRequestV2::decode(
                bytes
                    .get(..bytes.len() - 1)
                    .expect("one-byte-short request"),
            ),
            Err(BatchErrorV2::InvalidLength)
        );
    }

    #[test]
    fn receipt_binds_cache_request_and_every_current_deployment() {
        let observations = [
            observation(ExecutionRoleV1::Core, 1),
            observation(ExecutionRoleV1::Claims, 2),
            observation(ExecutionRoleV1::Trading, 3),
            observation(ExecutionRoleV1::Custody, 5),
        ];
        let input = RoleBatchReceiptInputV2 {
            registry_program: program(90),
            activation_cache: id(91),
            activation_cache_digest: content(92),
            release_set_id: content(93),
            request_digest: content(94),
            observations: &observations,
        };
        let mut bytes = [0xa5; ROLE_BATCH_RECEIPT_BYTES_V2];
        encode_role_batch_receipt_v2(input, &mut bytes).expect("receipt");
        let receipt = AuthenticatedRoleBatchReceiptV2::decode(&bytes).expect("decode");
        assert_eq!(receipt.registry_program(), input.registry_program);
        assert_eq!(receipt.activation_cache(), input.activation_cache);
        assert_eq!(
            receipt.activation_cache_digest(),
            input.activation_cache_digest
        );
        assert_eq!(receipt.release_set_id(), input.release_set_id);
        assert_eq!(receipt.request_digest(), input.request_digest);
        assert_eq!(receipt.role_count(), 4);
        assert_eq!(receipt.role_mask(), 0b1_0111);
        for (index, expected) in observations.into_iter().enumerate() {
            assert_eq!(receipt.observation(index), Some(Ok(expected)));
        }
        assert_eq!(receipt.observation(4), None);
    }

    #[test]
    fn receipt_refuses_reorder_substitution_and_noncanonical_inactive_slots() {
        let observations = [
            observation(ExecutionRoleV1::Core, 1),
            observation(ExecutionRoleV1::Claims, 2),
        ];
        let input = RoleBatchReceiptInputV2 {
            registry_program: program(90),
            activation_cache: id(91),
            activation_cache_digest: content(92),
            release_set_id: content(93),
            request_digest: content(94),
            observations: &observations,
        };
        let mut bytes = [0; ROLE_BATCH_RECEIPT_BYTES_V2];
        encode_role_batch_receipt_v2(input, &mut bytes).expect("receipt");
        let mut reordered = bytes;
        reordered[RECEIPT_OBSERVATIONS_OFFSET] = role_tag(ExecutionRoleV1::Trading);
        assert_eq!(
            AuthenticatedRoleBatchReceiptV2::decode(&reordered),
            Err(BatchErrorV2::NonCanonicalRoleOrder)
        );
        for offset in [
            RECEIPT_REGISTRY_OFFSET,
            RECEIPT_CACHE_OFFSET,
            RECEIPT_CACHE_DIGEST_OFFSET,
            RECEIPT_RELEASE_SET_OFFSET,
            RECEIPT_REQUEST_DIGEST_OFFSET,
            RECEIPT_OBSERVATIONS_OFFSET + OBSERVATION_PROGRAM_OFFSET,
            RECEIPT_OBSERVATIONS_OFFSET + OBSERVATION_PROGRAMDATA_OFFSET,
            RECEIPT_OBSERVATIONS_OFFSET + OBSERVATION_ARTIFACT_OFFSET,
            RECEIPT_OBSERVATIONS_OFFSET + OBSERVATION_SEMANTIC_OFFSET,
            RECEIPT_OBSERVATIONS_OFFSET + OBSERVATION_DEPLOYMENT_SLOT_OFFSET,
        ] {
            let mut substituted = bytes;
            *substituted.get_mut(offset).expect("substitution offset") ^= 1;
            assert!(AuthenticatedRoleBatchReceiptV2::decode(&substituted).is_ok());
            assert_ne!(substituted, bytes);
        }
        let inactive =
            RECEIPT_OBSERVATIONS_OFFSET + observations.len() * ROLE_BATCH_OBSERVATION_BYTES_V2;
        let mut dirty = bytes;
        *dirty.get_mut(inactive).expect("inactive observation") = 1;
        assert_eq!(
            AuthenticatedRoleBatchReceiptV2::decode(&dirty),
            Err(BatchErrorV2::NonCanonicalReserved)
        );
        let mut output = [0x5a; ROLE_BATCH_RECEIPT_BYTES_V2 - 1];
        assert_eq!(
            encode_role_batch_receipt_v2(input, &mut output),
            Err(BatchErrorV2::InvalidLength)
        );
        assert!(output.iter().all(|byte| *byte == 0x5a));
        assert_eq!(
            AuthenticatedRoleBatchReceiptV2::decode(
                bytes
                    .get(..bytes.len() - 1)
                    .expect("one-byte-short receipt"),
            ),
            Err(BatchErrorV2::InvalidLength)
        );
    }
}
