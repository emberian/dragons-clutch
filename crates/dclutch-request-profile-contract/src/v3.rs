//! RequestProfile V3 with one exact typed borrowed-witness suffix.
//!
//! V3 wraps an existing V1 projector. The V1 program continues to own every
//! byte it validates or projects; V3 admits one bounded nonempty suffix only
//! as an opaque request for one declared child role and wire pair. The common
//! executor must additionally prove that exactly one selected child route
//! borrows the complete suffix. This contract never interprets child payloads.

use super::{ProjectionRegistersV1, RequestProfileV1, project_atomic};

/// RequestProfile V3 magic.
pub const REQUEST_PROFILE_V3_MAGIC: [u8; 8] = *b"DCLTRP03";
/// RequestProfile V3 schema version.
pub const REQUEST_PROFILE_V3_SCHEMA_VERSION: u16 = 3;
/// RequestProfile V3 artifact profile.
pub const REQUEST_PROFILE_V3_ARTIFACT_PROFILE: u16 = 3;
/// Exact wrapper header width before the embedded V1 profile.
pub const REQUEST_PROFILE_V3_HEADER_BYTES: usize = 64;
/// Finalized-record schema label for the borrowed-witness successor.
pub const REQUEST_PROFILE_V3_SCHEMA_RELEASE_PREIMAGE: &[u8] =
    b"dclutch/schema/request-profile-v3-borrowed-witness-v1";
/// SHA-256 of [`REQUEST_PROFILE_V3_SCHEMA_RELEASE_PREIMAGE`].
pub const REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID: [u8; 32] = [
    0x8d, 0xea, 0xa3, 0xf3, 0x40, 0xd5, 0x05, 0x79, 0xb2, 0x9a, 0x0a, 0x00, 0x18, 0xa2, 0x92, 0x04,
    0xc7, 0x2f, 0x0f, 0x69, 0xae, 0xf3, 0x8d, 0x63, 0xa6, 0x4a, 0xa5, 0x8a, 0xfa, 0xf2, 0x93, 0x68,
];

const EMBEDDED_V1_BYTES_OFFSET: usize = 12;
const MINIMUM_WITNESS_BYTES_OFFSET: usize = 16;
const MAXIMUM_WITNESS_BYTES_OFFSET: usize = 20;
const CONSUMER_ROLE_OFFSET: usize = 24;
const CHILD_REQUEST_MAGIC_OFFSET: usize = 32;
const CHILD_RECEIPT_MAGIC_OFFSET: usize = 40;
const CHILD_RECEIPT_BYTES_OFFSET: usize = 48;

/// Fixed canonical child role consuming the complete suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BorrowedWitnessRoleV3 {
    /// Canonical Market Core.
    Core,
    /// Canonical Claims owner.
    Claims,
    /// Canonical Resolution owner.
    Resolution,
    /// Canonical Custody owner.
    Custody,
}

impl BorrowedWitnessRoleV3 {
    const fn encode(self) -> u8 {
        match self {
            Self::Core => 1,
            Self::Claims => 2,
            Self::Resolution => 3,
            Self::Custody => 4,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Core),
            2 => Ok(Self::Claims),
            3 => Ok(Self::Resolution),
            4 => Ok(Self::Custody),
            _ => Err(Error::InvalidPolicy),
        }
    }
}

/// Typed policy for exactly one opaque child-request suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedWitnessPolicyV3 {
    /// Inclusive minimum nonzero suffix width.
    pub minimum_bytes: u32,
    /// Inclusive maximum suffix width.
    pub maximum_bytes: u32,
    /// Current release-selected child role.
    pub consumer_role: BorrowedWitnessRoleV3,
    /// Exact first eight bytes of the child request.
    pub child_request_magic: [u8; 8],
    /// Exact first eight bytes of the immediate child receipt.
    pub child_receipt_magic: [u8; 8],
    /// Exact immediate receipt width.
    pub child_receipt_bytes: u32,
}

impl BorrowedWitnessPolicyV3 {
    fn validate(self) -> Result<()> {
        if self.minimum_bytes == 0
            || self.minimum_bytes > self.maximum_bytes
            || self.child_request_magic == [0; 8]
            || self.child_receipt_magic == [0; 8]
            || self.child_receipt_bytes == 0
        {
            return Err(Error::InvalidPolicy);
        }
        Ok(())
    }
}

/// Stable RequestProfile V3 refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Selected and independently authenticated identities differed or were zero.
    ProgramIdentityMismatch,
    /// Wrapper, embedded profile, request, or caller-owned bank had another width.
    InvalidLength,
    /// Magic, version, artifact profile, or reserved bytes were noncanonical.
    InvalidHeader,
    /// Embedded RequestProfile V1 refused.
    InvalidEmbeddedProfile,
    /// Witness bounds, typed wire identity, role, or receipt shape refused.
    InvalidPolicy,
    /// Complete request did not contain the declared exact prefix plus witness.
    InvalidWitness,
}

/// Result alias for RequestProfile V3 operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Borrowed hostile-decoded profiled-prefix plus witness policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestProfileV3<'a> {
    request: RequestProfileV1<'a>,
    policy: BorrowedWitnessPolicyV3,
    bytes: &'a [u8],
}

impl<'a> RequestProfileV3<'a> {
    /// Decode only after descriptor selection joins authenticated raw bytes.
    pub fn decode_selected(
        selected_program_id: [u8; 32],
        authenticated_program_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        if selected_program_id == [0; 32]
            || authenticated_program_id == [0; 32]
            || selected_program_id != authenticated_program_id
        {
            return Err(Error::ProgramIdentityMismatch);
        }
        Self::decode(bytes)
    }

    /// Hostile-decode the exact wrapper, policy, and embedded V1 profile.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < REQUEST_PROFILE_V3_HEADER_BYTES
            || bytes.get(..8) != Some(REQUEST_PROFILE_V3_MAGIC.as_slice())
            || read_u16(bytes, 8)? != REQUEST_PROFILE_V3_SCHEMA_VERSION
            || read_u16(bytes, 10)? != REQUEST_PROFILE_V3_ARTIFACT_PROFILE
            || !all_zero(bytes, 25, 7)?
            || !all_zero(bytes, 52, 12)?
        {
            return Err(Error::InvalidHeader);
        }
        let embedded_bytes = usize::try_from(read_u32(bytes, EMBEDDED_V1_BYTES_OFFSET)?)
            .map_err(|_| Error::InvalidLength)?;
        let expected = REQUEST_PROFILE_V3_HEADER_BYTES
            .checked_add(embedded_bytes)
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let policy = BorrowedWitnessPolicyV3 {
            minimum_bytes: read_u32(bytes, MINIMUM_WITNESS_BYTES_OFFSET)?,
            maximum_bytes: read_u32(bytes, MAXIMUM_WITNESS_BYTES_OFFSET)?,
            consumer_role: BorrowedWitnessRoleV3::decode(byte(bytes, CONSUMER_ROLE_OFFSET)?)?,
            child_request_magic: read_array(bytes, CHILD_REQUEST_MAGIC_OFFSET)?,
            child_receipt_magic: read_array(bytes, CHILD_RECEIPT_MAGIC_OFFSET)?,
            child_receipt_bytes: read_u32(bytes, CHILD_RECEIPT_BYTES_OFFSET)?,
        };
        policy.validate()?;
        let request = RequestProfileV1::decode(
            bytes
                .get(REQUEST_PROFILE_V3_HEADER_BYTES..)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|_| Error::InvalidEmbeddedProfile)?;
        Ok(Self {
            request,
            policy,
            bytes,
        })
    }

    /// Embedded exact prefix validator/projector.
    pub const fn request_profile(self) -> RequestProfileV1<'a> {
        self.request
    }

    /// Exact typed child-witness policy.
    pub const fn witness_policy(self) -> BorrowedWitnessPolicyV3 {
        self.policy
    }

    /// Borrow the complete finalized content preimage.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Split one complete request into the profiled prefix and opaque witness.
    pub fn split_request(self, tail_count: u32, request: &'a [u8]) -> Result<(&'a [u8], &'a [u8])> {
        let prefix_bytes = self
            .request
            .request_bytes(tail_count)
            .map_err(|_| Error::InvalidEmbeddedProfile)?;
        let witness_bytes = request
            .len()
            .checked_sub(prefix_bytes)
            .ok_or(Error::InvalidWitness)?;
        let witness_u32 = u32::try_from(witness_bytes).map_err(|_| Error::InvalidWitness)?;
        if witness_u32 < self.policy.minimum_bytes || witness_u32 > self.policy.maximum_bytes {
            return Err(Error::InvalidWitness);
        }
        let (prefix, witness) = request
            .split_at_checked(prefix_bytes)
            .ok_or(Error::InvalidWitness)?;
        if witness.get(..8) != Some(self.policy.child_request_magic.as_slice()) {
            return Err(Error::InvalidWitness);
        }
        Ok((prefix, witness))
    }

    /// Validate/project only the exact declared prefix atomically.
    pub fn project_prefix_atomic(
        self,
        tail_count: u32,
        complete_request: &'a [u8],
        registers: ProjectionRegistersV1<'_>,
    ) -> Result<()> {
        let (prefix, _) = self.split_request(tail_count, complete_request)?;
        project_atomic(self.request, tail_count, prefix, registers)
            .map_err(|_| Error::InvalidEmbeddedProfile)
    }
}

/// Encode one complete V3 wrapper atomically.
pub fn encode_request_profile_v3_atomic(
    embedded_v1: &[u8],
    policy: BorrowedWitnessPolicyV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    RequestProfileV1::decode(embedded_v1).map_err(|_| Error::InvalidEmbeddedProfile)?;
    policy.validate()?;
    let embedded_bytes = u32::try_from(embedded_v1.len()).map_err(|_| Error::InvalidLength)?;
    let expected = REQUEST_PROFILE_V3_HEADER_BYTES
        .checked_add(embedded_v1.len())
        .ok_or(Error::InvalidLength)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    put(scratch, 0, &REQUEST_PROFILE_V3_MAGIC)?;
    put(scratch, 8, &REQUEST_PROFILE_V3_SCHEMA_VERSION.to_le_bytes())?;
    put(
        scratch,
        10,
        &REQUEST_PROFILE_V3_ARTIFACT_PROFILE.to_le_bytes(),
    )?;
    put(
        scratch,
        EMBEDDED_V1_BYTES_OFFSET,
        &embedded_bytes.to_le_bytes(),
    )?;
    put(
        scratch,
        MINIMUM_WITNESS_BYTES_OFFSET,
        &policy.minimum_bytes.to_le_bytes(),
    )?;
    put(
        scratch,
        MAXIMUM_WITNESS_BYTES_OFFSET,
        &policy.maximum_bytes.to_le_bytes(),
    )?;
    *scratch
        .get_mut(CONSUMER_ROLE_OFFSET)
        .ok_or(Error::InvalidLength)? = policy.consumer_role.encode();
    put(
        scratch,
        CHILD_REQUEST_MAGIC_OFFSET,
        &policy.child_request_magic,
    )?;
    put(
        scratch,
        CHILD_RECEIPT_MAGIC_OFFSET,
        &policy.child_receipt_magic,
    )?;
    put(
        scratch,
        CHILD_RECEIPT_BYTES_OFFSET,
        &policy.child_receipt_bytes.to_le_bytes(),
    )?;
    put(scratch, REQUEST_PROFILE_V3_HEADER_BYTES, embedded_v1)?;
    SelfCheck::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

type SelfCheck<'a> = RequestProfileV3<'a>;

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn all_zero(bytes: &[u8], offset: usize, width: usize) -> Result<bool> {
    Ok(bytes
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|value| *value == 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::{
        RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1, ScalarRegisterV1,
        encode_request_profile_v1_atomic,
    };

    fn embedded_profile() -> [u8; 80] {
        let instructions = [
            RequestInstructionV1::require_u64(
                RequestCoordinateV1::fixed(0),
                u64::from_le_bytes(*b"PREFIX03"),
            ),
            RequestInstructionV1::project_u64(
                RequestCoordinateV1::fixed(8),
                ScalarRegisterV1::common(0),
            ),
        ];
        let mut scratch = [0; 80];
        let mut output = [0; 80];
        encode_request_profile_v1_atomic(
            RequestGeometryV1::new(16, 0, 1, 0, 1, 0),
            &instructions,
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("embedded V1");
        output
    }

    fn policy() -> BorrowedWitnessPolicyV3 {
        BorrowedWitnessPolicyV3 {
            minimum_bytes: 8,
            maximum_bytes: 16,
            consumer_role: BorrowedWitnessRoleV3::Claims,
            child_request_magic: *b"CHILD003",
            child_receipt_magic: *b"RECPT003",
            child_receipt_bytes: 376,
        }
    }

    fn wrapper() -> [u8; 144] {
        let embedded = embedded_profile();
        let mut scratch = [0; 144];
        let mut output = [0; 144];
        encode_request_profile_v3_atomic(&embedded, policy(), &mut scratch, &mut output)
            .expect("V3 wrapper");
        output
    }

    #[test]
    fn exact_prefix_projects_and_typed_suffix_remains_borrowed() {
        let bytes = wrapper();
        let profile =
            RequestProfileV3::decode_selected([9; 32], [9; 32], &bytes).expect("selected profile");
        let mut request = [0_u8; 24];
        request[..8].copy_from_slice(b"PREFIX03");
        request[8..16].copy_from_slice(&41_u64.to_le_bytes());
        request[16..].copy_from_slice(b"CHILD003");
        let input_scalars = [0];
        let input_identities = [[0; 32]];
        let mut scratch_scalars = [0];
        let mut scratch_identities = [[0; 32]];
        let mut output_scalars = [0];
        let mut output_identities = [[0; 32]];
        profile
            .project_prefix_atomic(
                0,
                &request,
                ProjectionRegistersV1 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            )
            .expect("project prefix");
        assert_eq!(output_scalars, [41]);
        assert_eq!(
            profile.split_request(0, &request).expect("split").1,
            b"CHILD003"
        );
    }

    #[test]
    fn missing_oversized_or_wrong_typed_witness_refuses_atomically() {
        let bytes = wrapper();
        let profile = RequestProfileV3::decode(&bytes).expect("profile");
        for request in [
            b"PREFIX03\x29\0\0\0\0\0\0\0".as_slice(),
            b"PREFIX03\x29\0\0\0\0\0\0\0WRONG003".as_slice(),
            b"PREFIX03\x29\0\0\0\0\0\0\0CHILD003xxxxxxxxx".as_slice(),
        ] {
            let input_scalars = [0];
            let input_identities = [[0; 32]];
            let mut scratch_scalars = [0];
            let mut scratch_identities = [[0; 32]];
            let mut output_scalars = [77];
            let mut output_identities = [[7; 32]];
            assert!(
                profile
                    .project_prefix_atomic(
                        0,
                        request,
                        ProjectionRegistersV1 {
                            input_scalars: &input_scalars,
                            input_identities: &input_identities,
                            scratch_scalars: &mut scratch_scalars,
                            scratch_identities: &mut scratch_identities,
                            output_scalars: &mut output_scalars,
                            output_identities: &mut output_identities,
                        },
                    )
                    .is_err()
            );
            assert_eq!(output_scalars, [77]);
            assert_eq!(output_identities, [[7; 32]]);
        }
    }

    #[test]
    fn policy_header_and_content_selection_are_hostile() {
        let bytes = wrapper();
        assert_eq!(
            RequestProfileV3::decode_selected([1; 32], [2; 32], &bytes),
            Err(Error::ProgramIdentityMismatch)
        );
        let mut hostile = bytes;
        hostile[25] = 1;
        assert_eq!(
            RequestProfileV3::decode(&hostile),
            Err(Error::InvalidHeader)
        );
        hostile = bytes;
        hostile[MINIMUM_WITNESS_BYTES_OFFSET..MINIMUM_WITNESS_BYTES_OFFSET + 4]
            .copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            RequestProfileV3::decode(&hostile),
            Err(Error::InvalidPolicy)
        );
    }
}
