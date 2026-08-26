//! Registry-authenticated continuation request and ephemeral signer seeds.
//!
//! The Registry authenticates one canonical ordered role batch, then invokes
//! exactly one selected role with the bytes committed here.  The admission
//! PDA is never an account or persistent capability: its signer privilege
//! exists only in that nested invocation stack.

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{EXECUTION_ROLE_COUNT_V1, ExecutionRoleV1};

use crate::batch_v2::{BatchErrorV2, RoleBatchRequestV2};

/// Exact fixed header preceding the opaque continuation bytes.
pub const REGISTRY_CONTINUATION_REQUEST_BYTES_V1: usize = 128;
/// Canonical Registry continuation request magic.
pub const REGISTRY_CONTINUATION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLRGCI1";
/// Implemented Registry continuation wire schema.
pub const REGISTRY_CONTINUATION_SCHEMA_V1: u16 = 1;
/// PDA domain for the invocation-scoped Registry admission signer.
pub const REGISTRY_CONTINUATION_ADMISSION_DOMAIN_V1: &[u8] = b"dclutch:registry-continuation:v1";
/// Exact ordered Registry role batch for common Trading Hot execution.
pub const CORE_TRADING_HOT_CONTINUATION_ROLES_V1: [ExecutionRoleV1; 2] =
    [ExecutionRoleV1::Core, ExecutionRoleV1::Trading];

const COUNT_OFFSET: usize = 10;
const MASK_OFFSET: usize = 11;
const CONTINUATION_ROLE_OFFSET: usize = 12;
const RESERVED_HEADER_OFFSET: usize = 13;
const RELEASE_SET_OFFSET: usize = 16;
const CACHE_DIGEST_OFFSET: usize = 48;
const CONTINUATION_DIGEST_OFFSET: usize = 80;
const CONTINUATION_LEN_OFFSET: usize = 112;
const ROLES_OFFSET: usize = 116;
const RESERVED_TAIL_OFFSET: usize = 121;

/// One hostile-validated fixed Registry continuation header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryContinuationRequestV1 {
    release_set_id: ContentId,
    activation_cache_digest: ContentId,
    continuation_digest: ContentId,
    continuation_len: u32,
    continuation_role: ExecutionRoleV1,
    role_count: u8,
    role_mask: u8,
    roles: [u8; EXECUTION_ROLE_COUNT_V1],
}

impl RegistryContinuationRequestV1 {
    /// Construct one exact nonempty continuation over a canonical role batch.
    pub fn new(
        release_set_id: ContentId,
        activation_cache_digest: ContentId,
        continuation_digest: ContentId,
        continuation_len: u32,
        continuation_role: ExecutionRoleV1,
        roles: &[ExecutionRoleV1],
    ) -> Result<Self, BatchErrorV2> {
        if continuation_len == 0 {
            return Err(BatchErrorV2::InvalidLength);
        }
        let batch = RoleBatchRequestV2::new(release_set_id, activation_cache_digest, roles)?;
        if !roles.contains(&continuation_role) {
            return Err(BatchErrorV2::UnknownRole);
        }
        let mut encoded = [0_u8; EXECUTION_ROLE_COUNT_V1];
        for (index, role) in roles.iter().copied().enumerate() {
            *encoded
                .get_mut(index)
                .ok_or(BatchErrorV2::InvalidRoleCount)? = role_tag(role);
        }
        Ok(Self {
            release_set_id,
            activation_cache_digest,
            continuation_digest,
            continuation_len,
            continuation_role,
            role_count: batch.role_count(),
            role_mask: batch.role_mask(),
            roles: encoded,
        })
    }

    /// Construct the sole canonical Core+Trading top-level Hot continuation.
    pub fn new_core_trading_hot(
        release_set_id: ContentId,
        activation_cache_digest: ContentId,
        hot_instruction_digest: ContentId,
        hot_instruction_len: u32,
    ) -> Result<Self, BatchErrorV2> {
        Self::new(
            release_set_id,
            activation_cache_digest,
            hot_instruction_digest,
            hot_instruction_len,
            ExecutionRoleV1::Trading,
            &CORE_TRADING_HOT_CONTINUATION_ROLES_V1,
        )
    }

    /// Decode one exact canonical fixed header.
    pub fn decode(bytes: &[u8]) -> Result<Self, BatchErrorV2> {
        if bytes.len() != REGISTRY_CONTINUATION_REQUEST_BYTES_V1
            || bytes.get(..8) != Some(REGISTRY_CONTINUATION_REQUEST_MAGIC_V1.as_slice())
            || u16_at(bytes, 8)? != REGISTRY_CONTINUATION_SCHEMA_V1
        {
            return Err(BatchErrorV2::UnsupportedSchema);
        }
        require_zero(bytes, RESERVED_HEADER_OFFSET, 3)?;
        require_zero(bytes, RESERVED_TAIL_OFFSET, 7)?;
        let count = usize::from(byte(bytes, COUNT_OFFSET)?);
        if count == 0 || count > EXECUTION_ROLE_COUNT_V1 {
            return Err(BatchErrorV2::InvalidRoleCount);
        }
        let encoded = slice(bytes, ROLES_OFFSET, EXECUTION_ROLE_COUNT_V1)?;
        require_zero(encoded, count, EXECUTION_ROLE_COUNT_V1 - count)?;
        let mut roles = [ExecutionRoleV1::Core; EXECUTION_ROLE_COUNT_V1];
        for (index, tag) in encoded.iter().copied().take(count).enumerate() {
            *roles.get_mut(index).ok_or(BatchErrorV2::InvalidRoleCount)? = decode_role(tag)?;
        }
        let value = Self::new(
            content_id(bytes, RELEASE_SET_OFFSET)?,
            content_id(bytes, CACHE_DIGEST_OFFSET)?,
            content_id(bytes, CONTINUATION_DIGEST_OFFSET)?,
            u32_at(bytes, CONTINUATION_LEN_OFFSET)?,
            decode_role(byte(bytes, CONTINUATION_ROLE_OFFSET)?)?,
            roles.get(..count).ok_or(BatchErrorV2::InvalidRoleCount)?,
        )?;
        if value.role_count != byte(bytes, COUNT_OFFSET)?
            || value.role_mask != byte(bytes, MASK_OFFSET)?
        {
            return Err(BatchErrorV2::RoleMaskMismatch);
        }
        Ok(value)
    }

    /// Encode the fixed request header.
    pub fn to_bytes(self) -> [u8; REGISTRY_CONTINUATION_REQUEST_BYTES_V1] {
        let mut output = [0_u8; REGISTRY_CONTINUATION_REQUEST_BYTES_V1];
        put(&mut output, 0, &REGISTRY_CONTINUATION_REQUEST_MAGIC_V1);
        put(
            &mut output,
            8,
            &REGISTRY_CONTINUATION_SCHEMA_V1.to_le_bytes(),
        );
        output[COUNT_OFFSET] = self.role_count;
        output[MASK_OFFSET] = self.role_mask;
        output[CONTINUATION_ROLE_OFFSET] = role_tag(self.continuation_role);
        put(
            &mut output,
            RELEASE_SET_OFFSET,
            self.release_set_id.as_bytes(),
        );
        put(
            &mut output,
            CACHE_DIGEST_OFFSET,
            self.activation_cache_digest.as_bytes(),
        );
        put(
            &mut output,
            CONTINUATION_DIGEST_OFFSET,
            self.continuation_digest.as_bytes(),
        );
        put(
            &mut output,
            CONTINUATION_LEN_OFFSET,
            &self.continuation_len.to_le_bytes(),
        );
        put(&mut output, ROLES_OFFSET, &self.roles);
        output
    }

    /// Reconstruct the sole canonical batch request authenticated by Registry.
    pub fn role_batch_request(self) -> Result<RoleBatchRequestV2, BatchErrorV2> {
        let mut roles = [ExecutionRoleV1::Core; EXECUTION_ROLE_COUNT_V1];
        let count = usize::from(self.role_count);
        for index in 0..count {
            *roles.get_mut(index).ok_or(BatchErrorV2::InvalidRoleCount)? = decode_role(
                *self
                    .roles
                    .get(index)
                    .ok_or(BatchErrorV2::InvalidRoleCount)?,
            )?;
        }
        RoleBatchRequestV2::new(
            self.release_set_id,
            self.activation_cache_digest,
            roles.get(..count).ok_or(BatchErrorV2::InvalidRoleCount)?,
        )
    }

    /// Exact selected release set.
    pub const fn release_set_id(self) -> ContentId {
        self.release_set_id
    }

    /// Expected digest of the complete activation-cache bytes.
    pub const fn activation_cache_digest(self) -> ContentId {
        self.activation_cache_digest
    }

    /// Digest of the exact opaque continuation bytes.
    pub const fn continuation_digest(self) -> ContentId {
        self.continuation_digest
    }

    /// Exact opaque continuation byte count.
    pub const fn continuation_len(self) -> u32 {
        self.continuation_len
    }

    /// Selected role invoked by Registry.
    pub const fn continuation_role(self) -> ExecutionRoleV1 {
        self.continuation_role
    }

    /// Number of authenticated roles.
    pub const fn role_count(self) -> u8 {
        self.role_count
    }

    /// Exact canonical authenticated-role mask.
    pub const fn role_mask(self) -> u8 {
        self.role_mask
    }

    /// Read one role from the canonical ordered batch.
    pub fn role(self, index: usize) -> Option<ExecutionRoleV1> {
        if index >= usize::from(self.role_count) {
            return None;
        }
        self.roles.get(index).and_then(|tag| decode_role(*tag).ok())
    }

    /// Verify the exact typed Core+Trading Hot profile and every recomputed
    /// coordinate. The caller must independently hash the complete cache and
    /// byte-exact Hot instruction before calling this helper.
    pub fn verify_core_trading_hot(
        self,
        release_set_id: ContentId,
        activation_cache_digest: ContentId,
        hot_instruction_digest: ContentId,
        hot_instruction_len: u32,
    ) -> Result<(), BatchErrorV2> {
        if self.release_set_id != release_set_id
            || self.activation_cache_digest != activation_cache_digest
            || self.continuation_digest != hot_instruction_digest
            || self.continuation_len != hot_instruction_len
            || self.continuation_role != ExecutionRoleV1::Trading
            || usize::from(self.role_count) != CORE_TRADING_HOT_CONTINUATION_ROLES_V1.len()
            || CORE_TRADING_HOT_CONTINUATION_ROLES_V1
                .iter()
                .enumerate()
                .any(|(index, role)| self.role(index) != Some(*role))
        {
            return Err(BatchErrorV2::ContinuationProfileMismatch);
        }
        Ok(())
    }
}

/// Cycle-free seed projection for one invocation-scoped Registry signer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryContinuationAdmissionSeedsV1 {
    release_set_id: ContentId,
    activation_cache: [u8; 32],
    batch_request_digest: ContentId,
    role_mask: [u8; 1],
    continuation_role: [u8; 1],
    continuation_digest: ContentId,
}

impl RegistryContinuationAdmissionSeedsV1 {
    /// Construct exact seeds after the adapter independently hashes the batch
    /// request and continuation bytes.
    pub fn new(
        request: RegistryContinuationRequestV1,
        activation_cache: [u8; 32],
        batch_request_digest: ContentId,
    ) -> Result<Self, BatchErrorV2> {
        if activation_cache.iter().all(|byte| *byte == 0) {
            return Err(BatchErrorV2::ZeroIdentity);
        }
        Ok(Self {
            release_set_id: request.release_set_id,
            activation_cache,
            batch_request_digest,
            role_mask: [request.role_mask],
            continuation_role: [role_tag(request.continuation_role)],
            continuation_digest: request.continuation_digest,
        })
    }

    /// PDA domain.
    pub const fn domain(self) -> &'static [u8] {
        REGISTRY_CONTINUATION_ADMISSION_DOMAIN_V1
    }

    /// Exact release-set bytes.
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set_id.to_bytes()
    }

    /// Exact activation-cache account key.
    pub const fn activation_cache(self) -> [u8; 32] {
        self.activation_cache
    }

    /// SHA-256 digest of the exact canonical batch request.
    pub const fn batch_request_digest(self) -> [u8; 32] {
        self.batch_request_digest.to_bytes()
    }

    /// One-byte exact role mask seed.
    pub const fn role_mask(self) -> [u8; 1] {
        self.role_mask
    }

    /// One-byte selected continuation-role seed.
    pub const fn continuation_role(self) -> [u8; 1] {
        self.continuation_role
    }

    /// SHA-256 digest of the exact continuation bytes.
    pub const fn continuation_digest(self) -> [u8; 32] {
        self.continuation_digest.to_bytes()
    }
}

fn decode_role(tag: u8) -> Result<ExecutionRoleV1, BatchErrorV2> {
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

fn content_id(bytes: &[u8], offset: usize) -> Result<ContentId, BatchErrorV2> {
    ContentId::new(array(bytes, offset)?).map_err(|_| BatchErrorV2::Identity)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8, BatchErrorV2> {
    bytes
        .get(offset)
        .copied()
        .ok_or(BatchErrorV2::InvalidLength)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, BatchErrorV2> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| BatchErrorV2::InvalidLength)?,
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, BatchErrorV2> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| BatchErrorV2::InvalidLength)?,
    ))
}

fn array(bytes: &[u8], offset: usize) -> Result<[u8; 32], BatchErrorV2> {
    slice(bytes, offset, 32)?
        .try_into()
        .map_err(|_| BatchErrorV2::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], BatchErrorV2> {
    let end = offset.checked_add(len).ok_or(BatchErrorV2::InvalidLength)?;
    bytes.get(offset..end).ok_or(BatchErrorV2::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<(), BatchErrorV2> {
    if slice(bytes, offset, len)?.iter().any(|byte| *byte != 0) {
        return Err(BatchErrorV2::NonCanonicalReserved);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    fn content(seed: u8) -> ContentId {
        ContentId::new([seed; 32]).expect("nonzero content")
    }

    fn digest(bytes: &[u8]) -> ContentId {
        ContentId::new(Sha256::digest(bytes).into()).expect("nonzero digest")
    }

    #[test]
    fn canonical_header_and_cycle_free_seeds_bind_selected_role() {
        let roles = [
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ];
        let request = RegistryContinuationRequestV1::new(
            content(1),
            content(2),
            content(3),
            2_152,
            ExecutionRoleV1::Core,
            &roles,
        )
        .expect("request");
        assert_eq!(
            RegistryContinuationRequestV1::decode(&request.to_bytes()),
            Ok(request)
        );
        let batch = request.role_batch_request().expect("batch");
        let seeds =
            RegistryContinuationAdmissionSeedsV1::new(request, [4; 32], digest(&batch.to_bytes()))
                .expect("seeds");
        assert_eq!(seeds.role_mask(), [0b1_1011]);
        assert_eq!(seeds.continuation_role(), [0]);

        let claims = RegistryContinuationRequestV1::new(
            request.release_set_id(),
            request.activation_cache_digest(),
            request.continuation_digest(),
            request.continuation_len(),
            ExecutionRoleV1::Claims,
            &roles,
        )
        .expect("Claims continuation");
        let claims_seeds =
            RegistryContinuationAdmissionSeedsV1::new(claims, [4; 32], digest(&batch.to_bytes()))
                .expect("Claims seeds");
        assert_ne!(seeds.continuation_role(), claims_seeds.continuation_role());
    }

    #[test]
    fn core_trading_hot_profile_binds_order_role_and_exact_instruction() {
        let request = RegistryContinuationRequestV1::new_core_trading_hot(
            content(1),
            content(2),
            content(3),
            4_096,
        )
        .expect("Core+Trading Hot continuation");
        assert_eq!(
            request.verify_core_trading_hot(content(1), content(2), content(3), 4_096),
            Ok(())
        );
        for hostile in [
            request.verify_core_trading_hot(content(9), content(2), content(3), 4_096),
            request.verify_core_trading_hot(content(1), content(9), content(3), 4_096),
            request.verify_core_trading_hot(content(1), content(2), content(9), 4_096),
            request.verify_core_trading_hot(content(1), content(2), content(3), 4_095),
        ] {
            assert_eq!(hostile, Err(BatchErrorV2::ContinuationProfileMismatch));
        }

        let core_only = RegistryContinuationRequestV1::new(
            content(1),
            content(2),
            content(3),
            4_096,
            ExecutionRoleV1::Core,
            &[ExecutionRoleV1::Core],
        )
        .expect("generic Core continuation");
        assert_eq!(
            core_only.verify_core_trading_hot(content(1), content(2), content(3), 4_096),
            Err(BatchErrorV2::ContinuationProfileMismatch)
        );
    }

    #[test]
    fn hostile_role_digest_length_and_reserved_bytes_refuse() {
        let roles = [ExecutionRoleV1::Core, ExecutionRoleV1::Claims];
        assert_eq!(
            RegistryContinuationRequestV1::new(
                content(1),
                content(2),
                content(3),
                1,
                ExecutionRoleV1::Custody,
                &roles,
            ),
            Err(BatchErrorV2::UnknownRole)
        );
        assert_eq!(
            RegistryContinuationRequestV1::new(
                content(1),
                content(2),
                content(3),
                0,
                ExecutionRoleV1::Core,
                &roles,
            ),
            Err(BatchErrorV2::InvalidLength)
        );
        let request = RegistryContinuationRequestV1::new(
            content(1),
            content(2),
            content(3),
            9,
            ExecutionRoleV1::Core,
            &roles,
        )
        .expect("request");
        let mut dirty = request.to_bytes();
        dirty[127] = 1;
        assert_eq!(
            RegistryContinuationRequestV1::decode(&dirty),
            Err(BatchErrorV2::NonCanonicalReserved)
        );
        let mut swapped = request.to_bytes();
        swapped[CONTINUATION_ROLE_OFFSET] = role_tag(ExecutionRoleV1::Custody);
        assert_eq!(
            RegistryContinuationRequestV1::decode(&swapped),
            Err(BatchErrorV2::UnknownRole)
        );
        assert_eq!(
            RegistryContinuationRequestV1::decode(&request.to_bytes()[..127]),
            Err(BatchErrorV2::UnsupportedSchema)
        );
    }
}
