//! Headerless Registry-authenticated Trading Hot continuation facts.
//!
//! The top-level Registry instruction data is the exact canonical Trading Hot
//! instruction data. There is no second serialized request: Registry derives
//! every authority coordinate below from the authenticated activation cache
//! and the byte-exact Hot body before creating the invocation-scoped signer.

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::ExecutionRoleV1;

use crate::{
    batch_v2::{BatchErrorV2, RoleBatchRequestV2},
    continuation_v1::{
        REGISTRY_CONTINUATION_ADMISSION_DOMAIN_V1, RegistryContinuationAdmissionSeedsV1,
        RegistryContinuationRequestV1,
    },
};
/// Exact ordered role batch authenticated for transparent Trading Hot.
pub const TRANSPARENT_HOT_ROLES_V2: [ExecutionRoleV1; 2] =
    [ExecutionRoleV1::Core, ExecutionRoleV1::Trading];
/// Exact selected continuation role.
pub const TRANSPARENT_HOT_CONTINUATION_ROLE_V2: ExecutionRoleV1 = ExecutionRoleV1::Trading;

/// Derived authority facts for one byte-exact headerless Trading Hot call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransparentHotContinuationV2 {
    release_set_id: ContentId,
    activation_cache_digest: ContentId,
    hot_instruction_digest: ContentId,
    hot_instruction_len: u32,
}

impl TransparentHotContinuationV2 {
    /// Construct the fixed Core+Trading continuation from independently
    /// authenticated cache and Hot observations.
    pub fn new(
        release_set_id: ContentId,
        activation_cache_digest: ContentId,
        hot_instruction_digest: ContentId,
        hot_instruction_len: u32,
    ) -> Result<Self, BatchErrorV2> {
        if hot_instruction_len == 0 {
            return Err(BatchErrorV2::InvalidLength);
        }
        let value = Self {
            release_set_id,
            activation_cache_digest,
            hot_instruction_digest,
            hot_instruction_len,
        };
        value.role_batch_request()?;
        Ok(value)
    }

    /// Reconstruct the sole canonical ordered Registry batch.
    pub fn role_batch_request(self) -> Result<RoleBatchRequestV2, BatchErrorV2> {
        self.as_v1_request()?.role_batch_request()
    }

    /// Reconstruct the established typed continuation authority from derived
    /// facts. V2 removes only its serialized header and creates no new signer
    /// domain.
    pub fn as_v1_request(self) -> Result<RegistryContinuationRequestV1, BatchErrorV2> {
        RegistryContinuationRequestV1::new_core_trading_hot(
            self.release_set_id,
            self.activation_cache_digest,
            self.hot_instruction_digest,
            self.hot_instruction_len,
        )
    }

    /// Exact release set selected by both cache and Hot envelope.
    pub const fn release_set_id(self) -> ContentId {
        self.release_set_id
    }

    /// Digest of the complete authenticated activation-cache bytes.
    pub const fn activation_cache_digest(self) -> ContentId {
        self.activation_cache_digest
    }

    /// Digest of the complete unchanged Hot bytes.
    pub const fn hot_instruction_digest(self) -> ContentId {
        self.hot_instruction_digest
    }

    /// Exact unchanged Hot byte count.
    pub const fn hot_instruction_len(self) -> u32 {
        self.hot_instruction_len
    }
}

/// Cycle-free seed projection for a headerless Trading Hot admission signer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransparentHotAdmissionSeedsV2 {
    release_set_id: ContentId,
    activation_cache: [u8; 32],
    batch_request_digest: ContentId,
    role_mask: [u8; 1],
    continuation_role: [u8; 1],
    hot_instruction_digest: ContentId,
}

impl TransparentHotAdmissionSeedsV2 {
    /// Construct exact seeds after independently hashing the canonical batch
    /// request and complete Hot instruction bytes.
    pub fn new(
        continuation: TransparentHotContinuationV2,
        activation_cache: [u8; 32],
        batch_request_digest: ContentId,
    ) -> Result<Self, BatchErrorV2> {
        if activation_cache.iter().all(|byte| *byte == 0) {
            return Err(BatchErrorV2::ZeroIdentity);
        }
        let established = RegistryContinuationAdmissionSeedsV1::new(
            continuation.as_v1_request()?,
            activation_cache,
            batch_request_digest,
        )?;
        Ok(Self {
            release_set_id: continuation.release_set_id,
            activation_cache,
            batch_request_digest,
            role_mask: established.role_mask(),
            continuation_role: established.continuation_role(),
            hot_instruction_digest: continuation.hot_instruction_digest,
        })
    }

    /// Established admission PDA domain; V2 removes only wire redundancy.
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

    /// SHA-256 digest of the exact canonical role-batch request.
    pub const fn batch_request_digest(self) -> [u8; 32] {
        self.batch_request_digest.to_bytes()
    }

    /// Exact canonical Core+Trading role mask.
    pub const fn role_mask(self) -> [u8; 1] {
        self.role_mask
    }

    /// Exact Trading continuation-role tag.
    pub const fn continuation_role(self) -> [u8; 1] {
        self.continuation_role
    }

    /// SHA-256 digest of the exact unchanged Hot bytes.
    pub const fn hot_instruction_digest(self) -> [u8; 32] {
        self.hot_instruction_digest.to_bytes()
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
    fn headerless_facts_reconstruct_exact_core_trading_authority() {
        let hot = b"DCLTHOT3exact-hot-envelope-and-family-request";
        let continuation = TransparentHotContinuationV2::new(
            content(1),
            content(2),
            digest(hot),
            u32::try_from(hot.len()).expect("Hot width"),
        )
        .expect("transparent continuation");
        let batch = continuation.role_batch_request().expect("role batch");
        assert_eq!(batch.role_count(), 2);
        assert_eq!(batch.role(0), Some(ExecutionRoleV1::Core));
        assert_eq!(batch.role(1), Some(ExecutionRoleV1::Trading));
        let seeds =
            TransparentHotAdmissionSeedsV2::new(continuation, [3; 32], digest(&batch.to_bytes()))
                .expect("admission seeds");
        assert_eq!(seeds.role_mask(), [0b00101]);
        assert_eq!(seeds.continuation_role(), [2]);
        assert_eq!(seeds.hot_instruction_digest(), digest(hot).to_bytes());
        assert_eq!(
            seeds.domain(),
            crate::continuation_v1::REGISTRY_CONTINUATION_ADMISSION_DOMAIN_V1
        );
    }

    #[test]
    fn zero_length_cache_alias_and_hot_substitution_refuse_or_separate() {
        assert_eq!(
            TransparentHotContinuationV2::new(content(1), content(2), content(3), 0),
            Err(BatchErrorV2::InvalidLength)
        );
        let first = TransparentHotContinuationV2::new(content(1), content(2), content(3), 128)
            .expect("first continuation");
        let second = TransparentHotContinuationV2::new(content(1), content(2), content(4), 128)
            .expect("changed continuation");
        let batch = first.role_batch_request().expect("role batch");
        assert_eq!(
            TransparentHotAdmissionSeedsV2::new(first, [0; 32], digest(&batch.to_bytes())),
            Err(BatchErrorV2::ZeroIdentity)
        );
        assert_ne!(
            TransparentHotAdmissionSeedsV2::new(first, [5; 32], digest(&batch.to_bytes()))
                .expect("first seeds"),
            TransparentHotAdmissionSeedsV2::new(second, [5; 32], digest(&batch.to_bytes()))
                .expect("changed seeds")
        );
    }
}
