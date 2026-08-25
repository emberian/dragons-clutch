#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout semantic contract for one immutable execution release set.
//!
//! A Market commits one content identity for the complete encoded value rather
//! than independently trusting program IDs scattered across capability
//! adapters.  Each role binds an exact program identity to the content identity
//! of its checked artifact release.  This crate deliberately performs no
//! hashing, Loader inspection, Solana account access, or release admission.

use dclutch_core_contract::ContentId;

/// Bytes in one program-identity or content-identity coordinate.
pub const IDENTITY_BYTES: usize = 32;
/// Number of semantic execution roles in profile 1.
pub const EXECUTION_ROLE_COUNT_V1: usize = 5;
/// Bytes in the canonical profile-1 header.
pub const EXECUTION_RELEASE_SET_HEADER_BYTES_V1: usize = 16;
/// Bytes in one exact `(program, artifact release)` binding.
pub const EXECUTION_ROLE_BINDING_BYTES_V1: usize = 2 * IDENTITY_BYTES;
/// Bytes in one exact profile-1 execution release set.
pub const EXECUTION_RELEASE_SET_BYTES_V1: usize = EXECUTION_RELEASE_SET_HEADER_BYTES_V1
    + EXECUTION_ROLE_COUNT_V1 * EXECUTION_ROLE_BINDING_BYTES_V1;
/// Canonical profile-1 wire magic.
pub const EXECUTION_RELEASE_SET_MAGIC_V1: [u8; 8] = *b"DCLTRLS1";
/// Implemented execution-release-set schema version.
pub const EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1: u16 = 1;
/// Implemented fixed-layout artifact profile.
pub const EXECUTION_RELEASE_SET_ARTIFACT_PROFILE_V1: u16 = 1;
/// Schema and validator release identity for profile 1.
///
/// This is SHA-256 of `dclutch/schema/execution-release-set-v1`.
pub const EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1: [u8; IDENTITY_BYTES] = [
    0x8b, 0xa3, 0xbc, 0x19, 0x7f, 0xea, 0xa1, 0x87, 0xa0, 0xa3, 0x92, 0x7b, 0x16, 0xb2, 0x5d, 0x83,
    0x79, 0x2c, 0x5f, 0x33, 0x5a, 0xf2, 0x43, 0x39, 0xa5, 0x4c, 0x38, 0xcc, 0x07, 0x23, 0x03, 0x58,
];

const SCHEMA_OFFSET: usize = 8;
const PROFILE_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 12;
const RESERVED_BYTES: usize = 4;
const PROGRAM_OFFSET: usize = 0;
const ARTIFACT_RELEASE_OFFSET: usize = IDENTITY_BYTES;

/// Stable refusal returned by the execution-release-set contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A byte slice did not have the one exact profile width.
    InvalidLength,
    /// Magic bytes did not identify an execution release set.
    InvalidMagic,
    /// The record named an unsupported semantic schema version.
    UnsupportedSchema,
    /// The record named an unsupported fixed-layout artifact profile.
    UnsupportedArtifactProfile,
    /// Header padding was not the canonical all-zero sequence.
    NonCanonicalReservedBytes,
    /// A role named the reserved all-zero program identity.
    ZeroProgramIdentity,
    /// A role named the reserved all-zero artifact-release content identity.
    ZeroArtifactReleaseId,
    /// Equal program or release identities did not identify the same pair.
    InconsistentAliasedRoleBinding,
}

/// Result alias for execution-release-set operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Opaque nonzero identity of one executable program account.
///
/// The owning adapter decides how these bytes are interpreted as a chain
/// address.  This contract assigns no Solana or loader semantics to them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct ProgramIdentityV1([u8; IDENTITY_BYTES]);

impl ProgramIdentityV1 {
    /// Validate and construct one program identity.
    pub fn new(bytes: [u8; IDENTITY_BYTES]) -> Result<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroProgramIdentity);
        }
        Ok(Self(bytes))
    }

    /// Decode one exact-width program identity.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::new(read_array(bytes, 0)?)
    }

    /// Return the exact identity bytes.
    pub const fn to_bytes(self) -> [u8; IDENTITY_BYTES] {
        self.0
    }

    /// Borrow the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; IDENTITY_BYTES] {
        &self.0
    }
}

/// Opaque nonzero content identity of one checked artifact release.
///
/// The referenced release is expected to bind Loader metadata and complete ELF
/// identity.  This type deliberately does not duplicate those facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct ArtifactReleaseIdV1(ContentId);

impl ArtifactReleaseIdV1 {
    /// Validate and construct one artifact-release content identity.
    pub fn new(bytes: [u8; IDENTITY_BYTES]) -> Result<Self> {
        ContentId::new(bytes)
            .map(Self)
            .map_err(|_| Error::ZeroArtifactReleaseId)
    }

    /// Decode one exact-width artifact-release content identity.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::new(read_array(bytes, 0)?)
    }

    /// Return the underlying opaque content identity.
    pub const fn content_id(self) -> ContentId {
        self.0
    }

    /// Return the exact identity bytes.
    pub const fn to_bytes(self) -> [u8; IDENTITY_BYTES] {
        self.0.to_bytes()
    }

    /// Borrow the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; IDENTITY_BYTES] {
        self.0.as_bytes()
    }
}

/// One exact execution-role binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionRoleBindingV1 {
    program: ProgramIdentityV1,
    artifact_release: ArtifactReleaseIdV1,
}

impl ExecutionRoleBindingV1 {
    /// Construct one already validated role binding.
    pub const fn new(program: ProgramIdentityV1, artifact_release: ArtifactReleaseIdV1) -> Self {
        Self {
            program,
            artifact_release,
        }
    }

    /// Return the exact executable program identity.
    pub const fn program(self) -> ProgramIdentityV1 {
        self.program
    }

    /// Return the checked artifact-release content identity.
    pub const fn artifact_release(self) -> ArtifactReleaseIdV1 {
        self.artifact_release
    }
}

/// Semantic execution role in the one canonical profile-1 order.
///
/// Roles may intentionally share one program, but only through one identical
/// `(program, artifact release)` pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExecutionRoleV1 {
    /// Immutable Realm, Product, Market, manifest, record, and release registry.
    Core = 0,
    /// Sole canonical owner of claim balances and reusable replay coordinates.
    Claims = 1,
    /// Admission controller for data-defined trading transitions.
    Trading = 2,
    /// Provider authentication and terminal outcome admission controller.
    Resolution = 3,
    /// Realm-selected token custody and physical transfer adapter.
    Custody = 4,
}

impl ExecutionRoleV1 {
    const fn index(self) -> usize {
        match self {
            Self::Core => 0,
            Self::Claims => 1,
            Self::Trading => 2,
            Self::Resolution => 3,
            Self::Custody => 4,
        }
    }
}

/// Immutable canonical profile-1 execution release set.
///
/// Hashing [`Self::to_bytes`] gives the one content identity a Market should
/// select after its capability profile is upgraded to understand this schema.
/// This type is the sole semantic owner of execution-role membership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionReleaseSetV1 {
    core: ExecutionRoleBindingV1,
    claims: ExecutionRoleBindingV1,
    trading: ExecutionRoleBindingV1,
    resolution: ExecutionRoleBindingV1,
    custody: ExecutionRoleBindingV1,
}

impl ExecutionReleaseSetV1 {
    /// Construct and validate one named execution release set.
    pub fn new(
        core: ExecutionRoleBindingV1,
        claims: ExecutionRoleBindingV1,
        trading: ExecutionRoleBindingV1,
        resolution: ExecutionRoleBindingV1,
        custody: ExecutionRoleBindingV1,
    ) -> Result<Self> {
        let bindings = [core, claims, trading, resolution, custody];
        validate_aliases(&bindings)?;
        Ok(Self {
            core,
            claims,
            trading,
            resolution,
            custody,
        })
    }

    /// Hostile-decode one exact canonical profile-1 value.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != EXECUTION_RELEASE_SET_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..EXECUTION_RELEASE_SET_MAGIC_V1.len())
            != Some(EXECUTION_RELEASE_SET_MAGIC_V1.as_slice())
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, PROFILE_OFFSET)? != EXECUTION_RELEASE_SET_ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        Self::new(
            decode_binding(bytes, ExecutionRoleV1::Core)?,
            decode_binding(bytes, ExecutionRoleV1::Claims)?,
            decode_binding(bytes, ExecutionRoleV1::Trading)?,
            decode_binding(bytes, ExecutionRoleV1::Resolution)?,
            decode_binding(bytes, ExecutionRoleV1::Custody)?,
        )
    }

    /// Encode the one exact canonical profile-1 byte sequence.
    pub fn to_bytes(self) -> [u8; EXECUTION_RELEASE_SET_BYTES_V1] {
        let mut output = [0u8; EXECUTION_RELEASE_SET_BYTES_V1];
        copy_infallible(&mut output, 0, &EXECUTION_RELEASE_SET_MAGIC_V1);
        put_u16(
            &mut output,
            SCHEMA_OFFSET,
            EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1,
        );
        put_u16(
            &mut output,
            PROFILE_OFFSET,
            EXECUTION_RELEASE_SET_ARTIFACT_PROFILE_V1,
        );
        for role in ALL_EXECUTION_ROLES_V1 {
            let binding = self.binding(role);
            let offset = binding_offset(role);
            copy_infallible(&mut output, offset, binding.program.as_bytes());
            copy_infallible(
                &mut output,
                offset + ARTIFACT_RELEASE_OFFSET,
                binding.artifact_release.as_bytes(),
            );
        }
        output
    }

    /// Return the exact binding for one semantic role.
    pub const fn binding(self, role: ExecutionRoleV1) -> ExecutionRoleBindingV1 {
        match role {
            ExecutionRoleV1::Core => self.core,
            ExecutionRoleV1::Claims => self.claims,
            ExecutionRoleV1::Trading => self.trading,
            ExecutionRoleV1::Resolution => self.resolution,
            ExecutionRoleV1::Custody => self.custody,
        }
    }
}

const ALL_EXECUTION_ROLES_V1: [ExecutionRoleV1; EXECUTION_ROLE_COUNT_V1] = [
    ExecutionRoleV1::Core,
    ExecutionRoleV1::Claims,
    ExecutionRoleV1::Trading,
    ExecutionRoleV1::Resolution,
    ExecutionRoleV1::Custody,
];

fn validate_aliases(bindings: &[ExecutionRoleBindingV1; EXECUTION_ROLE_COUNT_V1]) -> Result<()> {
    let mut left = 0usize;
    while left < EXECUTION_ROLE_COUNT_V1 {
        let mut right = left
            .checked_add(1)
            .ok_or(Error::InconsistentAliasedRoleBinding)?;
        while right < EXECUTION_ROLE_COUNT_V1 {
            let left_binding = bindings
                .get(left)
                .copied()
                .ok_or(Error::InconsistentAliasedRoleBinding)?;
            let right_binding = bindings
                .get(right)
                .copied()
                .ok_or(Error::InconsistentAliasedRoleBinding)?;
            let same_program = left_binding.program == right_binding.program;
            let same_release = left_binding.artifact_release == right_binding.artifact_release;
            if same_program != same_release {
                return Err(Error::InconsistentAliasedRoleBinding);
            }
            right = right
                .checked_add(1)
                .ok_or(Error::InconsistentAliasedRoleBinding)?;
        }
        left = left
            .checked_add(1)
            .ok_or(Error::InconsistentAliasedRoleBinding)?;
    }
    Ok(())
}

fn binding_offset(role: ExecutionRoleV1) -> usize {
    EXECUTION_RELEASE_SET_HEADER_BYTES_V1 + role.index() * EXECUTION_ROLE_BINDING_BYTES_V1
}

fn decode_binding(bytes: &[u8], role: ExecutionRoleV1) -> Result<ExecutionRoleBindingV1> {
    let offset = binding_offset(role);
    Ok(ExecutionRoleBindingV1::new(
        ProgramIdentityV1::decode(subslice(bytes, offset + PROGRAM_OFFSET, IDENTITY_BYTES)?)?,
        ArtifactReleaseIdV1::decode(subslice(
            bytes,
            offset + ARTIFACT_RELEASE_OFFSET,
            IDENTITY_BYTES,
        )?)?,
    ))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    subslice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn subslice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if subslice(bytes, offset, width)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn copy_infallible(output: &mut [u8], offset: usize, source: &[u8]) {
    let Some(end) = offset.checked_add(source.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(source);
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    copy_infallible(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    fn program(byte: u8) -> ProgramIdentityV1 {
        ProgramIdentityV1::new([byte; IDENTITY_BYTES]).expect("nonzero program")
    }

    fn release(byte: u8) -> ArtifactReleaseIdV1 {
        ArtifactReleaseIdV1::new([byte; IDENTITY_BYTES]).expect("nonzero release")
    }

    fn role(program_byte: u8, release_byte: u8) -> ExecutionRoleBindingV1 {
        ExecutionRoleBindingV1::new(program(program_byte), release(release_byte))
    }

    fn fixture() -> ExecutionReleaseSetV1 {
        ExecutionReleaseSetV1::new(
            role(1, 11),
            role(2, 12),
            role(3, 13),
            role(4, 14),
            role(5, 15),
        )
        .expect("canonical release set")
    }

    fn mutate(bytes: &mut [u8], offset: usize, value: u8) {
        *bytes.get_mut(offset).expect("fixture offset") = value;
    }

    #[test]
    fn exact_round_trip_preserves_every_named_role() {
        let release_set = fixture();
        let bytes = release_set.to_bytes();
        assert_eq!(EXECUTION_RELEASE_SET_BYTES_V1, 336);
        assert_eq!(bytes.len(), EXECUTION_RELEASE_SET_BYTES_V1);
        assert_eq!(
            [
                binding_offset(ExecutionRoleV1::Core),
                binding_offset(ExecutionRoleV1::Claims),
                binding_offset(ExecutionRoleV1::Trading),
                binding_offset(ExecutionRoleV1::Resolution),
                binding_offset(ExecutionRoleV1::Custody),
            ],
            [16, 80, 144, 208, 272]
        );
        assert_eq!(
            bytes.get(..EXECUTION_RELEASE_SET_HEADER_BYTES_V1),
            Some(
                [
                    b'D', b'C', b'L', b'T', b'R', b'L', b'S', b'1', 1, 0, 1, 0, 0, 0, 0, 0,
                ]
                .as_slice()
            )
        );
        assert_eq!(ExecutionReleaseSetV1::decode(&bytes), Ok(release_set));
        for (role, program_byte, release_byte) in [
            (ExecutionRoleV1::Core, 1, 11),
            (ExecutionRoleV1::Claims, 2, 12),
            (ExecutionRoleV1::Trading, 3, 13),
            (ExecutionRoleV1::Resolution, 4, 14),
            (ExecutionRoleV1::Custody, 5, 15),
        ] {
            assert_eq!(
                release_set.binding(role).program().to_bytes(),
                [program_byte; 32]
            );
            assert_eq!(
                release_set.binding(role).artifact_release().to_bytes(),
                [release_byte; 32]
            );
            let offset = binding_offset(role);
            assert_eq!(
                bytes.get(offset..offset + IDENTITY_BYTES),
                Some([program_byte; IDENTITY_BYTES].as_slice())
            );
            assert_eq!(
                bytes.get(
                    offset + ARTIFACT_RELEASE_OFFSET
                        ..offset + ARTIFACT_RELEASE_OFFSET + IDENTITY_BYTES
                ),
                Some([release_byte; IDENTITY_BYTES].as_slice())
            );
        }
        assert!(
            bytes
                .get(RESERVED_OFFSET..RESERVED_OFFSET + RESERVED_BYTES)
                .is_some_and(|reserved| reserved.iter().all(|byte| *byte == 0))
        );
    }

    #[test]
    fn exact_pair_aliases_allow_merged_program_roles() {
        let shared = role(7, 17);
        let release_set =
            ExecutionReleaseSetV1::new(role(1, 11), shared, shared, role(4, 14), role(5, 15))
                .expect("identical role pair may be shared");
        assert_eq!(
            release_set.binding(ExecutionRoleV1::Claims),
            release_set.binding(ExecutionRoleV1::Trading)
        );
        assert_eq!(
            ExecutionReleaseSetV1::decode(&release_set.to_bytes()),
            Ok(release_set)
        );
    }

    #[test]
    fn contradictory_program_or_release_aliases_refuse() {
        assert_eq!(
            ExecutionReleaseSetV1::new(
                role(1, 11),
                role(2, 12),
                role(2, 13),
                role(4, 14),
                role(5, 15),
            ),
            Err(Error::InconsistentAliasedRoleBinding)
        );
        assert_eq!(
            ExecutionReleaseSetV1::new(
                role(1, 11),
                role(2, 12),
                role(3, 12),
                role(4, 14),
                role(5, 15),
            ),
            Err(Error::InconsistentAliasedRoleBinding)
        );
    }

    #[test]
    fn widths_magic_versions_profile_and_reserved_bytes_are_hostile() {
        let bytes = fixture().to_bytes();
        assert_eq!(
            ExecutionReleaseSetV1::decode(bytes.get(..bytes.len() - 1).expect("nonempty fixture")),
            Err(Error::InvalidLength)
        );
        let mut extended = Vec::from(bytes);
        extended.push(0);
        assert_eq!(
            ExecutionReleaseSetV1::decode(&extended),
            Err(Error::InvalidLength)
        );

        for (offset, error) in [
            (0, Error::InvalidMagic),
            (SCHEMA_OFFSET, Error::UnsupportedSchema),
            (PROFILE_OFFSET, Error::UnsupportedArtifactProfile),
            (RESERVED_OFFSET, Error::NonCanonicalReservedBytes),
        ] {
            let mut hostile = bytes;
            mutate(&mut hostile, offset, 0xff);
            assert_eq!(ExecutionReleaseSetV1::decode(&hostile), Err(error));
        }
    }

    #[test]
    fn every_zero_program_and_release_coordinate_refuses() {
        let bytes = fixture().to_bytes();
        for role in ALL_EXECUTION_ROLES_V1 {
            let mut zero_program = bytes;
            zero_program
                .get_mut(binding_offset(role)..binding_offset(role) + IDENTITY_BYTES)
                .expect("program span")
                .fill(0);
            assert_eq!(
                ExecutionReleaseSetV1::decode(&zero_program),
                Err(Error::ZeroProgramIdentity)
            );

            let release_offset = binding_offset(role) + ARTIFACT_RELEASE_OFFSET;
            let mut zero_release = bytes;
            zero_release
                .get_mut(release_offset..release_offset + IDENTITY_BYTES)
                .expect("release span")
                .fill(0);
            assert_eq!(
                ExecutionReleaseSetV1::decode(&zero_release),
                Err(Error::ZeroArtifactReleaseId)
            );
        }
    }

    #[test]
    fn hostile_wire_aliases_refuse_after_individual_identity_validation() {
        let mut same_program = fixture().to_bytes();
        let claims_program = binding_offset(ExecutionRoleV1::Claims);
        let trading_program = binding_offset(ExecutionRoleV1::Trading);
        let source: [u8; IDENTITY_BYTES] = same_program
            .get(claims_program..claims_program + IDENTITY_BYTES)
            .expect("claims program")
            .try_into()
            .expect("identity width");
        same_program
            .get_mut(trading_program..trading_program + IDENTITY_BYTES)
            .expect("trading program")
            .copy_from_slice(&source);
        assert_eq!(
            ExecutionReleaseSetV1::decode(&same_program),
            Err(Error::InconsistentAliasedRoleBinding)
        );

        let mut same_release = fixture().to_bytes();
        let claims_release = claims_program + ARTIFACT_RELEASE_OFFSET;
        let trading_release = trading_program + ARTIFACT_RELEASE_OFFSET;
        let source: [u8; IDENTITY_BYTES] = same_release
            .get(claims_release..claims_release + IDENTITY_BYTES)
            .expect("claims release")
            .try_into()
            .expect("identity width");
        same_release
            .get_mut(trading_release..trading_release + IDENTITY_BYTES)
            .expect("trading release")
            .copy_from_slice(&source);
        assert_eq!(
            ExecutionReleaseSetV1::decode(&same_release),
            Err(Error::InconsistentAliasedRoleBinding)
        );
    }
}
