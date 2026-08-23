//! Typed, resumable transport for immutable protocol artifacts.
//!
//! A transport account is not an artifact and can never be consumed as one.
//! It is an uploader-scoped, program-owned staging area whose header commits
//! to one [`ArtifactBinding`] and whose body is filled strictly from left to
//! right in [`ARTIFACT_CHUNK_BYTES`] chunks.  Only a complete stage can be
//! validated through [`validate_artifact`].  The Solana adapter is responsible
//! for creating the final content-derived PDA, copying the validated bytes,
//! and closing the stage back to its recorded funder atomically.
//!
//! The module deliberately has no generic blob kind.  Every admitted kind has
//! one existing hostile-byte codec and one exact length.  Adding a source
//! specification, archive page, or clearing artifact therefore requires
//! adding its owning codec here first; callers cannot make an untyped upload
//! become consensus truth by choosing a new discriminant.

#[cfg(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point"
))]
use super::direct_selection_v3::DirectBatchPolicyV3;
use super::direct_selection_v3::DIRECT_BATCH_POLICY_V3_BYTES;
use super::{
    account_len, collateral, is_zero, CodecError, Hash32, PriceGridAccount, Result, TermsAccount,
    HASH_BYTES,
};
#[cfg(feature = "profile-direct-v3-source-v2-point")]
use clutch_batch_policy_identity::BATCH_POLICY_BYTES;
#[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy, Identity32V1, BATCH_POLICY_BYTES,
};

/// Stage-account discriminator.
pub const ARTIFACT_STAGE_TAG: u8 = 0x21;
/// First and only stage-account schema understood by this build.
pub const ARTIFACT_STAGE_VERSION: u8 = 1;
/// Bytes carried by every non-final upload write.
///
/// A write intent also carries the complete type/context/digest binding and
/// stays below the protocol's existing 310-byte intent ceiling.
pub const ARTIFACT_CHUNK_BYTES: usize = 192;
/// Reserved zero bytes at the end of the stage header.
pub const ARTIFACT_STAGE_RESERVED_BYTES: usize = 16;
/// Exact fixed header length before the staged artifact body.
pub const ARTIFACT_STAGE_HEADER_BYTES: usize = 2
    + 1
    + 1
    + 2
    + 2
    + 8
    + 8
    + HASH_BYTES
    + HASH_BYTES
    + HASH_BYTES
    + ARTIFACT_STAGE_RESERVED_BYTES;
/// Largest artifact body admitted by this transport revision.
pub const MAX_ARTIFACT_BYTES: usize = account_len::TERMS;

/// A fixed artifact family with one owning hostile-byte codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ArtifactKind {
    /// The Realm collateral policy. Its context is the parent Profile id.
    CollateralPolicy = 1,
    /// A frozen price grid. Its context is the Realm id.
    PriceGrid = 2,
    /// Immutable market terms. Its context is the Realm id.
    Terms = 3,
    /// Immutable full-width batch-policy preimage. Its context is an Epoch id.
    BatchPolicy = 4,
    /// Direct-policy plus verifier release identity. Its context is an Epoch id.
    DirectBatchPolicyV3 = 5,
}

impl ArtifactKind {
    /// Decode the stable wire discriminant.
    pub const fn from_byte(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::CollateralPolicy),
            2 => Ok(Self::PriceGrid),
            3 => Ok(Self::Terms),
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            4 => Ok(Self::BatchPolicy),
            #[cfg(any(
                feature = "profile-full",
                feature = "profile-direct-v3-source-v2-point"
            ))]
            5 => Ok(Self::DirectBatchPolicyV3),
            _ => Err(CodecError::InvalidEnum),
        }
    }

    /// Stable wire discriminant.
    pub const fn byte(self) -> u8 {
        self as u8
    }

    /// Exact canonical body length for this kind.
    pub const fn exact_len(self) -> usize {
        match self {
            Self::CollateralPolicy => collateral::COLLATERAL_POLICY_BYTES,
            Self::PriceGrid => account_len::PRICE_GRID,
            Self::Terms => account_len::TERMS,
            Self::BatchPolicy => BATCH_POLICY_BYTES,
            Self::DirectBatchPolicyV3 => DIRECT_BATCH_POLICY_V3_BYTES,
        }
    }
}

/// The immutable identity of one upload and its eventual final artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactBinding {
    /// Codec family.
    pub kind: ArtifactKind,
    /// Profile id for a collateral policy; Realm id for grid and terms.
    pub context: Hash32,
    /// Canonical semantic digest owned by the artifact codec.
    pub digest: Hash32,
    /// Exact byte length, redundantly checked against [`ArtifactKind`].
    pub exact_len: u16,
}

impl ArtifactBinding {
    /// Refuse zero identities, invented lengths, and bodies above the bound.
    pub fn validate(&self) -> Result<()> {
        if is_zero(&self.context.0) || is_zero(&self.digest.0) {
            return Err(CodecError::ZeroIdentity);
        }
        if self.exact_len as usize != self.kind.exact_len()
            || self.exact_len as usize > MAX_ARTIFACT_BYTES
        {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }
}

/// Decoded header of one in-progress artifact upload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactStageHeader {
    /// Immutable artifact identity.
    pub binding: ArtifactBinding,
    /// Wallet that created, funds, writes, seals, and may abort the stage.
    pub funder: [u8; HASH_BYTES],
    /// First byte not yet written.
    pub cursor: u16,
    /// Slot at which the stage was created.
    pub created_slot: u64,
    /// Last slot at which a write or seal is admitted.
    pub expires_slot: u64,
    /// Canonical staging-PDA bump.
    pub stored_bump: u8,
}

impl ArtifactStageHeader {
    /// Total account size required for this stage.
    pub fn account_len(&self) -> Result<usize> {
        self.validate()?;
        ARTIFACT_STAGE_HEADER_BYTES
            .checked_add(self.binding.exact_len as usize)
            .ok_or(CodecError::ArithmeticOverflow)
    }

    /// Refuse impossible upload geometry or authority/time metadata.
    pub fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        if is_zero(&self.funder) {
            return Err(CodecError::ZeroIdentity);
        }
        if self.created_slot >= self.expires_slot {
            return Err(CodecError::InvalidCount);
        }
        if self.cursor > self.binding.exact_len {
            return Err(CodecError::InvalidCount);
        }
        if self.cursor != self.binding.exact_len
            && usize::from(self.cursor) % ARTIFACT_CHUNK_BYTES != 0
        {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }

    /// Whether every artifact byte has been admitted.
    pub const fn is_complete(&self) -> bool {
        self.cursor == self.binding.exact_len
    }
}

fn put_u16(out: &mut [u8], at: &mut usize, value: u16) {
    out[*at..*at + 2].copy_from_slice(&value.to_le_bytes());
    *at += 2;
}

fn put_u64(out: &mut [u8], at: &mut usize, value: u64) {
    out[*at..*at + 8].copy_from_slice(&value.to_le_bytes());
    *at += 8;
}

fn take_u16(input: &[u8], at: &mut usize) -> u16 {
    let value = u16::from_le_bytes([input[*at], input[*at + 1]]);
    *at += 2;
    value
}

fn take_u64(input: &[u8], at: &mut usize) -> u64 {
    let value = u64::from_le_bytes([
        input[*at],
        input[*at + 1],
        input[*at + 2],
        input[*at + 3],
        input[*at + 4],
        input[*at + 5],
        input[*at + 6],
        input[*at + 7],
    ]);
    *at += 8;
    value
}

fn copy_32(input: &[u8], at: &mut usize) -> [u8; HASH_BYTES] {
    let mut value = [0; HASH_BYTES];
    value.copy_from_slice(&input[*at..*at + HASH_BYTES]);
    *at += HASH_BYTES;
    value
}

fn encode_header_prefix(out: &mut [u8], header: &ArtifactStageHeader) -> Result<()> {
    header.validate()?;
    if out.len() < ARTIFACT_STAGE_HEADER_BYTES {
        return Err(CodecError::OutputTooSmall);
    }
    let mut at = 0;
    out[at] = ARTIFACT_STAGE_TAG;
    at += 1;
    out[at] = ARTIFACT_STAGE_VERSION;
    at += 1;
    out[at] = header.binding.kind.byte();
    at += 1;
    out[at] = header.stored_bump;
    at += 1;
    put_u16(out, &mut at, header.binding.exact_len);
    put_u16(out, &mut at, header.cursor);
    put_u64(out, &mut at, header.created_slot);
    put_u64(out, &mut at, header.expires_slot);
    out[at..at + HASH_BYTES].copy_from_slice(&header.funder);
    at += HASH_BYTES;
    out[at..at + HASH_BYTES].copy_from_slice(&header.binding.context.0);
    at += HASH_BYTES;
    out[at..at + HASH_BYTES].copy_from_slice(&header.binding.digest.0);
    at += HASH_BYTES;
    out[at..at + ARTIFACT_STAGE_RESERVED_BYTES].fill(0);
    at += ARTIFACT_STAGE_RESERVED_BYTES;
    if at != ARTIFACT_STAGE_HEADER_BYTES {
        return Err(CodecError::OutputTooSmall);
    }
    Ok(())
}

/// Initialize an exact-size staging account, including its canonical zero tail.
pub fn initialize_stage(out: &mut [u8], header: &ArtifactStageHeader) -> Result<()> {
    if out.len() != header.account_len()? {
        return Err(CodecError::OutputTooSmall);
    }
    out.fill(0);
    encode_header_prefix(out, header)
}

/// Decode and fully validate a staging account.
///
/// Bytes beyond the cursor must still be zero.  This is redundant for a
/// normally program-owned account but makes hostile genesis fixtures and
/// corrupted state fail closed rather than masquerade as unwritten space.
pub fn decode_stage(input: &[u8]) -> Result<ArtifactStageHeader> {
    if input.len() < ARTIFACT_STAGE_HEADER_BYTES {
        return Err(CodecError::Truncated);
    }
    if input[0] != ARTIFACT_STAGE_TAG {
        return Err(CodecError::WrongTag);
    }
    if input[1] != ARTIFACT_STAGE_VERSION {
        return Err(CodecError::WrongVersion);
    }
    let mut at = 2;
    let kind = ArtifactKind::from_byte(input[at])?;
    at += 1;
    let stored_bump = input[at];
    at += 1;
    let exact_len = take_u16(input, &mut at);
    let cursor = take_u16(input, &mut at);
    let created_slot = take_u64(input, &mut at);
    let expires_slot = take_u64(input, &mut at);
    let funder = copy_32(input, &mut at);
    let context = Hash32::from_bytes(copy_32(input, &mut at));
    let digest = Hash32::from_bytes(copy_32(input, &mut at));
    let reserved = &input[at..at + ARTIFACT_STAGE_RESERVED_BYTES];
    at += ARTIFACT_STAGE_RESERVED_BYTES;
    if reserved.iter().any(|byte| *byte != 0) {
        return Err(CodecError::NonCanonicalPadding);
    }
    if at != ARTIFACT_STAGE_HEADER_BYTES {
        return Err(CodecError::TrailingBytes);
    }
    let header = ArtifactStageHeader {
        binding: ArtifactBinding {
            kind,
            context,
            digest,
            exact_len,
        },
        funder,
        cursor,
        created_slot,
        expires_slot,
        stored_bump,
    };
    if input.len() != header.account_len()? {
        return Err(CodecError::TrailingBytes);
    }
    let unwritten = ARTIFACT_STAGE_HEADER_BYTES + usize::from(header.cursor);
    if input[unwritten..].iter().any(|byte| *byte != 0) {
        return Err(CodecError::NonCanonicalPadding);
    }
    Ok(header)
}

/// Return the complete or partial payload after validating the whole stage.
pub fn stage_payload(input: &[u8]) -> Result<&[u8]> {
    decode_stage(input)?;
    Ok(&input[ARTIFACT_STAGE_HEADER_BYTES..])
}

/// Append exactly the next fixed-size chunk, or the unique shorter final one.
///
/// Duplicate chunks, gaps, overlaps, mixed artifact bindings, nonzero wire
/// padding, and writes after completion all refuse before any byte changes.
pub fn append_chunk(
    stage: &mut [u8],
    binding: ArtifactBinding,
    expected_cursor: u16,
    chunk_len: u16,
    chunk: &[u8; ARTIFACT_CHUNK_BYTES],
) -> Result<ArtifactStageHeader> {
    let mut header = decode_stage(stage)?;
    binding.validate()?;
    if header.binding != binding || header.cursor != expected_cursor || header.is_complete() {
        return Err(CodecError::MismatchedBinding);
    }
    let remaining = usize::from(header.binding.exact_len - header.cursor);
    let required = if remaining < ARTIFACT_CHUNK_BYTES {
        remaining
    } else {
        ARTIFACT_CHUNK_BYTES
    };
    if usize::from(chunk_len) != required {
        return Err(CodecError::InvalidCount);
    }
    if chunk[required..].iter().any(|byte| *byte != 0) {
        return Err(CodecError::NonCanonicalPadding);
    }
    let start = ARTIFACT_STAGE_HEADER_BYTES + usize::from(header.cursor);
    let end = start + required;
    stage[start..end].copy_from_slice(&chunk[..required]);
    header.cursor = header
        .cursor
        .checked_add(chunk_len)
        .ok_or(CodecError::ArithmeticOverflow)?;
    encode_header_prefix(stage, &header)?;
    Ok(header)
}

/// Validate a complete staged body through the existing owning codec.
///
/// Returns the final account's stored bump for grid/terms and zero for the raw
/// collateral policy, whose encoding intentionally contains no PDA field.
pub fn validate_artifact(binding: ArtifactBinding, body: &[u8]) -> Result<u8> {
    binding.validate()?;
    if body.len() != usize::from(binding.exact_len) {
        return Err(CodecError::Truncated);
    }
    match binding.kind {
        ArtifactKind::CollateralPolicy => {
            let policy = collateral::CollateralPolicy::decode(body)?;
            let digest = policy.digest()?;
            let parent =
                collateral::ParentProfile::from_policy_digest(digest, policy.schema_version)?;
            if digest != binding.digest || parent.identity()? != binding.context {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::PriceGrid => {
            let grid = PriceGridAccount::decode(body)?;
            if grid.realm != binding.context || grid.grid != binding.digest {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(grid.stored_bump)
        }
        ArtifactKind::Terms => {
            let terms = TermsAccount::decode(body)?;
            if terms.realm != binding.context || terms.terms != binding.digest {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(terms.stored_bump)
        }
        #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
        ArtifactKind::BatchPolicy => {
            let policy = decode_batch_policy(body).map_err(|_| CodecError::MismatchedBinding)?;
            let digest = batch_policy_digest(&policy).map_err(|_| CodecError::MismatchedBinding)?;
            if digest != Identity32V1(binding.digest.bytes()) {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(feature = "profile-direct-v3-source-v2-point")]
        ArtifactKind::BatchPolicy => Err(CodecError::InvalidEnum),
        #[cfg(any(
            feature = "profile-full",
            feature = "profile-direct-v3-source-v2-point"
        ))]
        ArtifactKind::DirectBatchPolicyV3 => {
            let policy = DirectBatchPolicyV3::decode(body)?;
            if policy.digest_for_epoch(binding.context)? != binding.digest {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(feature = "profile-general-source-v2-point")]
        ArtifactKind::DirectBatchPolicyV3 => Err(CodecError::InvalidEnum),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    use clutch_batch_policy_identity::{
        batch_policy_digest, direct_window_v1::DIRECT_POLICY_V1, encode_batch_policy,
    };
    extern crate std;

    fn binding(kind: ArtifactKind) -> ArtifactBinding {
        ArtifactBinding {
            kind,
            context: Hash32::from_bytes([0x31; 32]),
            digest: Hash32::from_bytes([0x52; 32]),
            exact_len: kind.exact_len() as u16,
        }
    }

    fn header(kind: ArtifactKind) -> ArtifactStageHeader {
        ArtifactStageHeader {
            binding: binding(kind),
            funder: [0x73; 32],
            cursor: 0,
            created_slot: 40,
            expires_slot: 400,
            stored_bump: 254,
        }
    }

    #[test]
    fn stage_lengths_and_round_trip_are_exact() {
        fn round_trip(kind: ArtifactKind) {
            let h = header(kind);
            let mut bytes = std::vec![0xa5; h.account_len().unwrap()];
            initialize_stage(&mut bytes, &h).unwrap();
            assert_eq!(decode_stage(&bytes), Ok(h));
            assert_eq!(
                stage_payload(&bytes).unwrap(),
                &bytes[ARTIFACT_STAGE_HEADER_BYTES..]
            );
            assert!(bytes[ARTIFACT_STAGE_HEADER_BYTES..]
                .iter()
                .all(|byte| *byte == 0));
        }
        for kind in [
            ArtifactKind::CollateralPolicy,
            ArtifactKind::PriceGrid,
            ArtifactKind::Terms,
        ] {
            round_trip(kind);
        }
        #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
        round_trip(ArtifactKind::BatchPolicy);
        #[cfg(any(
            feature = "profile-full",
            feature = "profile-direct-v3-source-v2-point"
        ))]
        round_trip(ArtifactKind::DirectBatchPolicyV3);
    }

    #[test]
    fn ordered_chunks_reject_every_ambiguity() {
        let h = header(ArtifactKind::CollateralPolicy);
        let mut bytes = std::vec![0; h.account_len().unwrap()];
        initialize_stage(&mut bytes, &h).unwrap();
        let mut first = [0; ARTIFACT_CHUNK_BYTES];
        first.fill(0x19);

        let before = bytes.clone();
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 1, 192, &first),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(bytes, before);

        let post = append_chunk(&mut bytes, h.binding, 0, 192, &first).unwrap();
        assert_eq!(post.cursor, 192);
        let after_first = bytes.clone();
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 0, 192, &first),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(bytes, after_first);

        let mut final_chunk = [0; ARTIFACT_CHUNK_BYTES];
        final_chunk[..74].fill(0x2a);
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 192, 73, &final_chunk),
            Err(CodecError::InvalidCount)
        );
        final_chunk[100] = 1;
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 192, 74, &final_chunk),
            Err(CodecError::NonCanonicalPadding)
        );
        final_chunk[100] = 0;
        assert!(append_chunk(&mut bytes, h.binding, 192, 74, &final_chunk)
            .unwrap()
            .is_complete());
        let complete = bytes.clone();
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 266, 0, &[0; ARTIFACT_CHUNK_BYTES]),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(bytes, complete);
    }

    #[test]
    fn hostile_stage_bytes_fail_closed() {
        let h = header(ArtifactKind::Terms);
        let mut bytes = std::vec![0; h.account_len().unwrap()];
        initialize_stage(&mut bytes, &h).unwrap();

        let mut bad = bytes.clone();
        bad[0] ^= 1;
        assert_eq!(decode_stage(&bad), Err(CodecError::WrongTag));
        bad = bytes.clone();
        bad[1] = 9;
        assert_eq!(decode_stage(&bad), Err(CodecError::WrongVersion));
        bad = bytes.clone();
        bad[2] = 9;
        assert_eq!(decode_stage(&bad), Err(CodecError::InvalidEnum));
        bad = bytes.clone();
        bad[ARTIFACT_STAGE_HEADER_BYTES - 1] = 1;
        assert_eq!(decode_stage(&bad), Err(CodecError::NonCanonicalPadding));
        bad = bytes.clone();
        bad[ARTIFACT_STAGE_HEADER_BYTES + 1] = 1;
        assert_eq!(decode_stage(&bad), Err(CodecError::NonCanonicalPadding));
        assert_eq!(
            decode_stage(&bytes[..bytes.len() - 1]),
            Err(CodecError::TrailingBytes)
        );
    }

    #[test]
    fn invented_kinds_lengths_and_times_refuse() {
        assert_eq!(ArtifactKind::from_byte(0), Err(CodecError::InvalidEnum));
        let mut h = header(ArtifactKind::Terms);
        h.binding.exact_len -= 1;
        assert_eq!(h.validate(), Err(CodecError::InvalidCount));
        h = header(ArtifactKind::Terms);
        h.expires_slot = h.created_slot;
        assert_eq!(h.validate(), Err(CodecError::InvalidCount));
        h = header(ArtifactKind::Terms);
        h.cursor = 1;
        assert_eq!(h.validate(), Err(CodecError::InvalidCount));
    }

    #[test]
    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    fn batch_policy_artifact_uses_the_canonical_policy_codec() {
        let mut bytes = [0u8; BATCH_POLICY_BYTES];
        assert_eq!(
            encode_batch_policy(&DIRECT_POLICY_V1, &mut bytes),
            Ok(BATCH_POLICY_BYTES)
        );
        let digest = batch_policy_digest(&DIRECT_POLICY_V1).unwrap();
        let binding = ArtifactBinding {
            kind: ArtifactKind::BatchPolicy,
            context: Hash32::from_bytes([0x44; 32]),
            digest: Hash32::from_bytes(digest.0),
            exact_len: BATCH_POLICY_BYTES as u16,
        };
        assert_eq!(validate_artifact(binding, &bytes), Ok(0));
        let mut hostile = bytes;
        hostile[12] ^= 1;
        assert_eq!(
            validate_artifact(binding, &hostile),
            Err(CodecError::MismatchedBinding)
        );
        let substituted = ArtifactBinding {
            digest: Hash32::from_bytes([0x55; 32]),
            ..binding
        };
        assert_eq!(
            validate_artifact(substituted, &bytes),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    #[cfg(any(
        feature = "profile-full",
        feature = "profile-direct-v3-source-v2-point"
    ))]
    fn direct_batch_policy_artifact_binds_kind_context_release_and_all_bytes() {
        let context = Hash32::from_bytes([0x44; 32]);
        let value = DirectBatchPolicyV3::direct(Hash32::from_bytes([0x77; 32])).unwrap();
        let mut bytes = [0u8; DIRECT_BATCH_POLICY_V3_BYTES];
        value.encode(&mut bytes).unwrap();
        let binding = ArtifactBinding {
            kind: ArtifactKind::DirectBatchPolicyV3,
            context,
            digest: value.digest_for_epoch(context).unwrap(),
            exact_len: DIRECT_BATCH_POLICY_V3_BYTES as u16,
        };
        assert_eq!(validate_artifact(binding, &bytes), Ok(0));

        let old_kind = ArtifactBinding {
            kind: ArtifactKind::BatchPolicy,
            exact_len: BATCH_POLICY_BYTES as u16,
            ..binding
        };
        #[cfg(feature = "profile-full")]
        assert_eq!(
            validate_artifact(old_kind, &bytes[..BATCH_POLICY_BYTES]),
            Err(CodecError::MismatchedBinding)
        );
        #[cfg(feature = "profile-direct-v3-source-v2-point")]
        assert_eq!(
            validate_artifact(old_kind, &bytes[..BATCH_POLICY_BYTES]),
            Err(CodecError::InvalidEnum)
        );
        let substituted_context = ArtifactBinding {
            context: Hash32::from_bytes([0x45; 32]),
            ..binding
        };
        assert_eq!(
            validate_artifact(substituted_context, &bytes),
            Err(CodecError::MismatchedBinding)
        );
        let mut hostile = bytes;
        hostile[DIRECT_BATCH_POLICY_V3_BYTES - 1] ^= 1;
        assert_eq!(
            validate_artifact(binding, &hostile),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            validate_artifact(binding, &bytes[..DIRECT_BATCH_POLICY_V3_BYTES - 1]),
            Err(CodecError::Truncated)
        );
    }

    #[test]
    #[cfg(feature = "profile-direct-v3-source-v2-point")]
    fn direct_profile_refuses_general_artifact_kind() {
        assert_eq!(ArtifactKind::from_byte(4), Err(CodecError::InvalidEnum));
    }

    #[test]
    #[cfg(feature = "profile-general-source-v2-point")]
    fn general_profile_refuses_direct_artifact_kind() {
        assert_eq!(ArtifactKind::from_byte(5), Err(CodecError::InvalidEnum));
    }
}
