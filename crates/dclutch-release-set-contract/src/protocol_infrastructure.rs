//! Immutable bootstrap selection for Registry and Rent infrastructure.

use crate::{
    ArtifactReleaseIdV1, Error, ExecutionRoleBindingV1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_RESERVED_OFFSET_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1, ProgramIdentityV1, Result,
};

const HEADER_RESERVED_BYTES: usize = 4;

/// Canonical finalized-record schema label for [`ProtocolInfrastructureProfileV1`].
pub const PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/protocol-infrastructure-profile-v1";
/// SHA-256 of [`PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_PREIMAGE_V1`].
pub const PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1: [u8; 32] = [
    0x45, 0x0e, 0xec, 0xe4, 0x59, 0x23, 0x30, 0x55, 0xce, 0x9a, 0xba, 0xd5, 0xbf, 0xce, 0x89, 0x83,
    0x80, 0xeb, 0xad, 0x75, 0xde, 0x0a, 0x16, 0x87, 0xbf, 0x77, 0xce, 0xd2, 0xa7, 0xae, 0xef, 0x8d,
];
/// Per-Core PDA seed domain for the immutable infrastructure profile.
pub const PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1: &[u8] = b"dclutch:infrastructure:v1";

const _: () = assert!(PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1.len() <= 32);

/// Immutable per-Core selection of exact Registry and Rent artifact releases.
///
/// The current Core ProgramData upgrade-authority signer selects this value
/// once.  Artifact records remain the sole owners of Loader and ELF facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolInfrastructureProfileV1 {
    registry: ExecutionRoleBindingV1,
    rent: ExecutionRoleBindingV1,
}

impl ProtocolInfrastructureProfileV1 {
    /// Construct one non-aliased infrastructure selection.
    pub fn new(registry: ExecutionRoleBindingV1, rent: ExecutionRoleBindingV1) -> Result<Self> {
        if registry.program() == rent.program()
            || registry.artifact_release() == rent.artifact_release()
        {
            return Err(Error::AliasedInfrastructureBinding);
        }
        Ok(Self { registry, rent })
    }

    /// Hostile-decode one exact canonical profile.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if slice(bytes, PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V1, 8)?
            != PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(
            bytes,
            PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1,
        )? != PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(
            bytes,
            PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1,
        )? != PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1
        {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(
            bytes,
            PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1,
            HEADER_RESERVED_BYTES,
        )?;
        Self::new(
            binding(
                bytes,
                PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1,
                PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1,
            )?,
            binding(
                bytes,
                PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1,
                PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1,
            )?,
        )
    }

    /// Encode the exact canonical profile preimage.
    pub fn to_bytes(self) -> [u8; PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1] {
        let mut output = [0; PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1];
        put(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V1,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1,
        );
        put(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1.to_le_bytes(),
        );
        put(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1.to_le_bytes(),
        );
        put_binding(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1,
            PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1,
            self.registry,
        );
        put_binding(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1,
            PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1,
            self.rent,
        );
        output
    }

    /// Return the exact selected Registry binding.
    pub const fn registry(self) -> ExecutionRoleBindingV1 {
        self.registry
    }

    /// Return the exact selected Rent binding.
    pub const fn rent(self) -> ExecutionRoleBindingV1 {
        self.rent
    }
}

/// Fixed instruction selecting initialization from authenticated artifact accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializeProtocolInfrastructureV1;

impl InitializeProtocolInfrastructureV1 {
    /// Hostile-decode the one canonical fixed initialization instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if slice(bytes, INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V1, 8)?
            != INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V1
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(
            bytes,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V1,
        )? != PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(
            bytes,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V1,
        )? != PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1
        {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(
            bytes,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_RESERVED_OFFSET_V1,
            HEADER_RESERVED_BYTES,
        )?;
        Ok(Self)
    }

    /// Encode the one canonical fixed initialization instruction.
    pub fn to_bytes(self) -> [u8; INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1] {
        let mut output = [0; INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1];
        put(
            &mut output,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V1,
            &INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V1,
        );
        put(
            &mut output,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V1,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1.to_le_bytes(),
        );
        put(
            &mut output,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V1,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1.to_le_bytes(),
        );
        output
    }
}

fn binding(
    bytes: &[u8],
    program_offset: usize,
    artifact_offset: usize,
) -> Result<ExecutionRoleBindingV1> {
    Ok(ExecutionRoleBindingV1::new(
        ProgramIdentityV1::decode(slice(bytes, program_offset, 32)?)?,
        ArtifactReleaseIdV1::decode(slice(bytes, artifact_offset, 32)?)?,
    ))
}

fn put_binding(
    output: &mut [u8],
    program_offset: usize,
    artifact_offset: usize,
    binding: ExecutionRoleBindingV1,
) {
    put(output, program_offset, binding.program().as_bytes());
    put(
        output,
        artifact_offset,
        binding.artifact_release().as_bytes(),
    );
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    let Some(end) = offset.checked_add(source.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(source);
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    fn binding_fixture(program: u8, artifact: u8) -> ExecutionRoleBindingV1 {
        ExecutionRoleBindingV1::new(
            ProgramIdentityV1::new([program; 32]).expect("program"),
            ArtifactReleaseIdV1::new([artifact; 32]).expect("artifact"),
        )
    }

    fn fixture() -> ProtocolInfrastructureProfileV1 {
        ProtocolInfrastructureProfileV1::new(binding_fixture(1, 2), binding_fixture(3, 4))
            .expect("profile")
    }

    #[test]
    fn profile_roundtrips_exact_lean_owned_wire() {
        let profile = fixture();
        let bytes = profile.to_bytes();
        assert_eq!(bytes.len(), 144);
        assert_eq!(ProtocolInfrastructureProfileV1::decode(&bytes), Ok(profile));
        assert_eq!(
            InitializeProtocolInfrastructureV1::decode(
                &InitializeProtocolInfrastructureV1.to_bytes(),
            ),
            Ok(InitializeProtocolInfrastructureV1),
        );
    }

    #[test]
    fn width_magic_version_profile_reserved_and_zero_identities_refuse() {
        let bytes = fixture().to_bytes();
        assert_eq!(
            ProtocolInfrastructureProfileV1::decode(&bytes[..143]),
            Err(Error::InvalidLength),
        );
        let mut extended = Vec::from(bytes);
        extended.push(0);
        assert_eq!(
            ProtocolInfrastructureProfileV1::decode(&extended),
            Err(Error::InvalidLength),
        );
        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1,
                Error::UnsupportedSchema,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1,
                Error::UnsupportedArtifactProfile,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1,
                Error::NonCanonicalReservedBytes,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1,
                Error::ZeroProgramIdentity,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1,
                Error::ZeroArtifactReleaseId,
            ),
        ] {
            let mut hostile = bytes;
            if matches!(
                expected,
                Error::ZeroProgramIdentity | Error::ZeroArtifactReleaseId
            ) {
                hostile
                    .get_mut(offset..offset + 32)
                    .expect("identity span")
                    .fill(0);
            } else {
                let byte = hostile.get_mut(offset).expect("hostile offset");
                *byte ^= 0xff;
            }
            assert_eq!(
                ProtocolInfrastructureProfileV1::decode(&hostile),
                Err(expected)
            );
        }
    }

    #[test]
    fn registry_and_rent_bindings_must_be_distinct() {
        assert_eq!(
            ProtocolInfrastructureProfileV1::new(binding_fixture(1, 2), binding_fixture(1, 3),),
            Err(Error::AliasedInfrastructureBinding),
        );
        assert_eq!(
            ProtocolInfrastructureProfileV1::new(binding_fixture(1, 2), binding_fixture(3, 2),),
            Err(Error::AliasedInfrastructureBinding),
        );
    }
}
