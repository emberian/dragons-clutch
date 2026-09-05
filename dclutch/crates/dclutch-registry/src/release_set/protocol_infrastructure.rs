//! Immutable bootstrap selection for Registry and Rent infrastructure.

use crate::release_set::{
    ArtifactReleaseIdV1, Error, ExecutionRoleBindingV1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V2,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1, INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V2,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V2,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V1, INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V2,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_RESERVED_OFFSET_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_RESERVED_OFFSET_V2,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_REGISTRY_ARTIFACT_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_RENT_ARTIFACT_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_TAIL_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V2, ProgramIdentityV1, Result,
};

const HEADER_RESERVED_BYTES: usize = 4;
const PROFILE_RESERVED_TAIL_BYTES_V2: usize = PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
    - PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_TAIL_OFFSET_V2;

/// Canonical finalized-record schema label for [`ProtocolInfrastructureProfileV1`].
pub const PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/protocol-infrastructure-profile-v1";
/// SHA-256 of [`PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_PREIMAGE_V1`].
pub const PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1: [u8; 32] = [
    0x45, 0x0e, 0xec, 0xe4, 0x59, 0x23, 0x30, 0x55, 0xce, 0x9a, 0xba, 0xd5, 0xbf, 0xce, 0x89, 0x83,
    0x80, 0xeb, 0xad, 0x75, 0xde, 0x0a, 0x16, 0x87, 0xbf, 0x77, 0xce, 0xd2, 0xa7, 0xae, 0xef, 0x8d,
];
// Both PDA seed domains now derive from
// `EmitProtocolInfrastructureProfileAbiRust`, re-exported through `lib.rs`
// alongside every other coordinate of this profile. Lean's
// `pda_domains_are_admissible_single_seeds` is the AUTHOR of the 32-byte seed
// bound that the hand-written `const _: () = assert!(..)` used to carry, and
// it adds what the bound alone never said: that the two domains differ, which
// is the whole reason V2 is a separate profile rather than a mutation of V1.
//
// The emitter restates that bound as a `const _: () = assert!(..)` beside each
// emitted domain, so it is also held where `cargo check` reads it. Proving it
// in Lean and dropping it from the Rust left the guard visible only to
// `check-generated.sh`, which needs `lake` and is the tier that skips on a
// host without one -- and the seam register's own class 1 is a ratchet on
// exactly this: an unguarded domain is the next over-length one.

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

/// Canonical finalized-record schema label for [`ProtocolInfrastructureProfileV2`].
pub const PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/protocol-infrastructure-profile-v2";
/// SHA-256 of [`PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_PREIMAGE_V2`].
pub const PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V2: [u8; 32] = [
    0xd3, 0x94, 0xf4, 0xd8, 0xa7, 0xcd, 0xf5, 0xe7, 0x7e, 0x97, 0x0d, 0x33, 0x0c, 0x74, 0x23, 0x6a,
    0x8f, 0x4b, 0x5c, 0x0a, 0x48, 0x5d, 0xbb, 0x79, 0xa1, 0x55, 0x5f, 0x31, 0x05, 0xad, 0x8b, 0x52,
];
/// Preimage of the genesis predecessor sentinel for the Registry binding.
pub const PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_PREIMAGE_V2: &[u8] =
    b"dclutch/genesis/protocol-infrastructure-predecessor-registry-v2";
/// SHA-256 of [`PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_PREIMAGE_V2`].
///
/// A cohort that succeeds nothing has no predecessor artifact release to name,
/// and cannot say so with zeros: [`ArtifactReleaseIdV1::new`] refuses an
/// all-zero id, and [`ProtocolInfrastructureProfileV2::new`] refuses two equal
/// predecessors as aliased. Two distinct domain-separated digests say "no
/// predecessor" in the one vocabulary the existing constructor already
/// accepts, so the genesis profile needs no new field and no new length.
///
/// These are not artifact releases and can never collide with one: a real
/// predecessor id is the SHA-256 of a published `ArtifactReleaseV1` record
/// body, and these are the SHA-256 of a fixed ASCII domain string that is not
/// a record body.
pub const PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2: [u8; 32] = [
    0x6c, 0x5e, 0x6d, 0x81, 0x92, 0x78, 0x03, 0x98, 0xa9, 0xc6, 0x8f, 0x06, 0x40, 0x9e, 0x80, 0xbf,
    0x2a, 0x2b, 0x2d, 0xde, 0xff, 0x86, 0x85, 0x2b, 0xa2, 0x7a, 0xc1, 0x4b, 0xa4, 0xbc, 0x8f, 0x06,
];
/// Preimage of the genesis predecessor sentinel for the Rent binding.
pub const PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_PREIMAGE_V2: &[u8] =
    b"dclutch/genesis/protocol-infrastructure-predecessor-rent-v2";
/// SHA-256 of [`PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_PREIMAGE_V2`].
pub const PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2: [u8; 32] = [
    0x3f, 0xf5, 0xe1, 0xb5, 0xde, 0x3c, 0xbd, 0x35, 0x2e, 0x22, 0xbd, 0x65, 0xc3, 0xc8, 0x7e, 0x53,
    0xf6, 0x77, 0x27, 0xe7, 0xca, 0xd1, 0x8d, 0x11, 0x67, 0x66, 0xe6, 0x79, 0x13, 0x84, 0xb9, 0x38,
];

const _: () = assert!(
    !konst_all_zero(&PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2)
        && !konst_all_zero(&PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2)
);
const _: () = assert!(!konst_equal(
    &PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2,
    &PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2,
));

// Both walk their slices by pattern rather than by index. `.get` is not
// available in a `const fn`, so an indexed loop here left `indexing_slicing` --
// denied workspace-wide -- with only a justified `#[allow]` as its other exit;
// walking leaves no bound to assert in the first place. Same shape as
// `dclutch-source`'s `states` (1bdf5572f).
const fn konst_all_zero(bytes: &[u8; 32]) -> bool {
    let mut rest: &[u8] = bytes;
    while let [head, tail @ ..] = rest {
        if *head != 0 {
            return false;
        }
        rest = tail;
    }
    true
}

const fn konst_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut left: &[u8] = left;
    let mut right: &[u8] = right;
    while let ([first, left_rest @ ..], [second, right_rest @ ..]) = (left, right) {
        if *first != *second {
            return false;
        }
        left = left_rest;
        right = right_rest;
    }
    true
}

// The V2 seed domain is emitted alongside V1 rather than written here: it was
// the last coordinate of this profile still stated by hand, and a domain is
// exactly the kind of string a transcription gets subtly wrong.

/// Succession selection of exact Registry and Rent artifact releases.
///
/// V1's two bindings plus the predecessor profile's two artifact-release ids,
/// making the succession content-walkable: V2 names exactly which V1 records
/// it succeeded, the way a release-lineage record keys its predecessor.
/// Created once by the succession ceremony
/// (`docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §5) under evidence
/// strictly stronger than V1's creation; redeployed consumers read V2 only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolInfrastructureProfileV2 {
    registry: ExecutionRoleBindingV1,
    rent: ExecutionRoleBindingV1,
    predecessor_registry_artifact: ArtifactReleaseIdV1,
    predecessor_rent_artifact: ArtifactReleaseIdV1,
}

impl ProtocolInfrastructureProfileV2 {
    /// Construct one non-aliased succession selection.
    ///
    /// The aliasing rules are V1's (registry and rent may share neither a
    /// program nor an artifact release) applied on both sides of the
    /// succession. Whether a binding was allowed to KEEP its predecessor's
    /// artifact id (the unmoved arm) or had to move it forward under consent
    /// is the ceremony's judgment, not this constructor's.
    pub fn new(
        registry: ExecutionRoleBindingV1,
        rent: ExecutionRoleBindingV1,
        predecessor_registry_artifact: ArtifactReleaseIdV1,
        predecessor_rent_artifact: ArtifactReleaseIdV1,
    ) -> Result<Self> {
        if registry.program() == rent.program()
            || registry.artifact_release() == rent.artifact_release()
            || predecessor_registry_artifact == predecessor_rent_artifact
        {
            return Err(Error::AliasedInfrastructureBinding);
        }
        Ok(Self {
            registry,
            rent,
            predecessor_registry_artifact,
            predecessor_rent_artifact,
        })
    }

    /// Hostile-decode one exact canonical succession profile.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if slice(bytes, PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V2, 8)?
            != PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V2
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(
            bytes,
            PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V2,
        )? != PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(
            bytes,
            PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V2,
        )? != PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1
        {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(
            bytes,
            PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V2,
            HEADER_RESERVED_BYTES,
        )?;
        require_zero(
            bytes,
            PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_TAIL_OFFSET_V2,
            PROFILE_RESERVED_TAIL_BYTES_V2,
        )?;
        Self::new(
            binding(
                bytes,
                PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V2,
                PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V2,
            )?,
            binding(
                bytes,
                PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V2,
                PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V2,
            )?,
            ArtifactReleaseIdV1::decode(slice(
                bytes,
                PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_REGISTRY_ARTIFACT_OFFSET_V2,
                32,
            )?)?,
            ArtifactReleaseIdV1::decode(slice(
                bytes,
                PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_RENT_ARTIFACT_OFFSET_V2,
                32,
            )?)?,
        )
    }

    /// Encode the exact canonical succession-profile preimage.
    pub fn to_bytes(self) -> [u8; PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2] {
        let mut output = [0; PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2];
        put(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V2,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V2,
        );
        put(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V2,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V2.to_le_bytes(),
        );
        put(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V2,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1.to_le_bytes(),
        );
        put_binding(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V2,
            PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V2,
            self.registry,
        );
        put_binding(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V2,
            PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V2,
            self.rent,
        );
        put(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_REGISTRY_ARTIFACT_OFFSET_V2,
            self.predecessor_registry_artifact.as_bytes(),
        );
        put(
            &mut output,
            PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_RENT_ARTIFACT_OFFSET_V2,
            self.predecessor_rent_artifact.as_bytes(),
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

    /// Return the predecessor profile's pinned Registry artifact-release id.
    pub const fn predecessor_registry_artifact(self) -> ArtifactReleaseIdV1 {
        self.predecessor_registry_artifact
    }

    /// Return the predecessor profile's pinned Rent artifact-release id.
    pub const fn predecessor_rent_artifact(self) -> ArtifactReleaseIdV1 {
        self.predecessor_rent_artifact
    }

    /// Construct the genesis succession profile for a cohort with no predecessor.
    ///
    /// Same two bindings a V1 would carry, and the two genesis sentinels in
    /// place of predecessor ids. The result is an ordinary V2 in every other
    /// respect, which is the point: every consumer keeps reading V2 and only
    /// V2, so `PROFILE_UPGRADE_RULING_2026_08_31.md` §6's "no fallback" holds
    /// with no second authentication path to forget.
    pub fn genesis(registry: ExecutionRoleBindingV1, rent: ExecutionRoleBindingV1) -> Result<Self> {
        Self::new(
            registry,
            rent,
            ArtifactReleaseIdV1::new(PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2)?,
            ArtifactReleaseIdV1::new(PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2)?,
        )
    }

    /// Was this profile BORN at V2, rather than succeeded from a V1?
    ///
    /// This is the whole no-fork rule, and it needs no new field: the
    /// distinction is already in the exact bytes the profile carries. A
    /// born-at-V2 cohort has not spent its one succession; a succeeded profile
    /// names the two real V1 artifact releases it moved forward from, and has.
    ///
    /// Soundness rests on who may write these bytes. Only Core writes a V2:
    /// genesis initialization writes the sentinels, and only into a vacant
    /// System-owned PDA; the ceremony writes real predecessor ids read out of
    /// the live V1 and can never write a sentinel. So a succeeded profile can
    /// never present as born-at-V2, and the rule cannot be forged from
    /// outside.
    ///
    /// **Both** sentinels are required. A profile carrying one sentinel and
    /// one real id is not a shape either writer can produce, so it is not
    /// genesis and it does not get a second succession.
    pub fn born_at_v2(self) -> bool {
        self.predecessor_registry_artifact.to_bytes()
            == PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2
            && self.predecessor_rent_artifact.to_bytes()
                == PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2
    }
}

/// Fixed instruction selecting the succession ceremony from authenticated accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializeProtocolInfrastructureV2;

impl InitializeProtocolInfrastructureV2 {
    /// Hostile-decode the one canonical fixed succession-ceremony instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if slice(bytes, INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V2, 8)?
            != INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V2
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(
            bytes,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V2,
        )? != PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(
            bytes,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V2,
        )? != PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1
        {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(
            bytes,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_RESERVED_OFFSET_V2,
            HEADER_RESERVED_BYTES,
        )?;
        Ok(Self)
    }

    /// Encode the one canonical fixed succession-ceremony instruction.
    pub fn to_bytes(self) -> [u8; INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V2] {
        let mut output = [0; INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V2];
        put(
            &mut output,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V2,
            &INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V2,
        );
        put(
            &mut output,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V2,
            &PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V2.to_le_bytes(),
        );
        put(
            &mut output,
            INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V2,
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

    /// A cohort with no predecessor gets a real, decodable V2 — which is the
    /// whole point of the shape: every consumer keeps reading V2 and only V2.
    #[test]
    fn a_genesis_profile_is_an_ordinary_v2_that_reports_itself_unspent() {
        let registry = binding_fixture(1, 2);
        let rent = binding_fixture(3, 4);
        let genesis =
            ProtocolInfrastructureProfileV2::genesis(registry, rent).expect("genesis profile");

        let bytes = genesis.to_bytes();
        assert_eq!(bytes.len(), PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2);
        assert_eq!(
            ProtocolInfrastructureProfileV2::decode(&bytes),
            Ok(genesis),
            "a genesis profile must survive the same hostile decode every V2 gets"
        );
        assert!(genesis.born_at_v2());
        assert_eq!(genesis.registry(), registry);
        assert_eq!(genesis.rent(), rent);
    }

    /// The control that makes the rule mean something: a SUCCEEDED profile,
    /// built the ordinary way, must not report itself unspent.
    #[test]
    fn a_succeeded_profile_has_spent_its_succession() {
        let succeeded = ProtocolInfrastructureProfileV2::new(
            binding_fixture(1, 9),
            binding_fixture(3, 8),
            ArtifactReleaseIdV1::new([2; 32]).expect("predecessor registry"),
            ArtifactReleaseIdV1::new([4; 32]).expect("predecessor rent"),
        )
        .expect("succeeded profile");
        assert!(!succeeded.born_at_v2());
    }

    /// Hostile: forge the sentinels onto a profile that has already succeeded.
    ///
    /// Half a forgery is still a forgery. A profile carrying one sentinel and
    /// one real predecessor id is not a shape either writer can produce, and it
    /// must not buy a second succession — so `born_at_v2` requires BOTH.
    #[test]
    fn a_half_forged_sentinel_pair_does_not_buy_a_second_succession() {
        let real_registry = ArtifactReleaseIdV1::new([2; 32]).expect("predecessor registry");
        let real_rent = ArtifactReleaseIdV1::new([4; 32]).expect("predecessor rent");
        let sentinel_registry =
            ArtifactReleaseIdV1::new(PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2)
                .expect("registry sentinel");
        let sentinel_rent =
            ArtifactReleaseIdV1::new(PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2)
                .expect("rent sentinel");

        for (predecessor_registry, predecessor_rent) in [
            (sentinel_registry, real_rent),
            (real_registry, sentinel_rent),
        ] {
            let forged = ProtocolInfrastructureProfileV2::new(
                binding_fixture(1, 9),
                binding_fixture(3, 8),
                predecessor_registry,
                predecessor_rent,
            )
            .expect("forged profile");
            assert!(
                !forged.born_at_v2(),
                "one sentinel and one real predecessor id is not a genesis profile"
            );
        }

        // And the positive control on the same fixture: both sentinels IS.
        let born = ProtocolInfrastructureProfileV2::new(
            binding_fixture(1, 9),
            binding_fixture(3, 8),
            sentinel_registry,
            sentinel_rent,
        )
        .expect("born-at-v2 profile");
        assert!(born.born_at_v2());
    }

    /// The sentinels must be constructible at all, which is the constraint that
    /// ruled out the obvious encodings: zeros are refused by `ContentId`, and an
    /// equal pair is refused as aliased.
    #[test]
    fn the_sentinels_are_nonzero_distinct_and_accepted_by_the_existing_constructor() {
        assert_ne!(
            PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2,
            PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2
        );
        assert_ne!(
            PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2,
            [0; 32]
        );
        assert_ne!(PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2, [0; 32]);
        assert!(ArtifactReleaseIdV1::new([0; 32]).is_err());
        let aliased = ArtifactReleaseIdV1::new([7; 32]).expect("artifact");
        assert!(
            ProtocolInfrastructureProfileV2::new(
                binding_fixture(1, 2),
                binding_fixture(3, 4),
                aliased,
                aliased,
            )
            .is_err(),
            "an equal predecessor pair stays aliased; the sentinels are distinct for this reason"
        );
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

    fn artifact_fixture(byte: u8) -> ArtifactReleaseIdV1 {
        ArtifactReleaseIdV1::new([byte; 32]).expect("artifact")
    }

    /// The canonical succession shape: the registry moved (5 succeeded 2), the
    /// rent binding carried its predecessor's artifact forward unchanged.
    fn fixture_v2() -> ProtocolInfrastructureProfileV2 {
        ProtocolInfrastructureProfileV2::new(
            binding_fixture(1, 5),
            binding_fixture(3, 4),
            artifact_fixture(2),
            artifact_fixture(4),
        )
        .expect("succession profile")
    }

    #[test]
    fn profile_v2_roundtrips_exact_lean_owned_wire() {
        let profile = fixture_v2();
        let bytes = profile.to_bytes();
        assert_eq!(bytes.len(), 224);
        assert_eq!(ProtocolInfrastructureProfileV2::decode(&bytes), Ok(profile));
        assert_eq!(
            InitializeProtocolInfrastructureV2::decode(
                &InitializeProtocolInfrastructureV2.to_bytes(),
            ),
            Ok(InitializeProtocolInfrastructureV2),
        );
    }

    /// V1's decoder and V2's refuse each other's wire: distinct magic, schema
    /// version, and width mean neither account can ever stand in for the other
    /// at a reader, which is what makes "V2-only, no fallback" enforceable.
    #[test]
    fn profile_versions_refuse_each_other() {
        assert_eq!(
            ProtocolInfrastructureProfileV1::decode(&fixture_v2().to_bytes()),
            Err(Error::InvalidLength),
        );
        assert_eq!(
            ProtocolInfrastructureProfileV2::decode(&fixture().to_bytes()),
            Err(Error::InvalidLength),
        );
        assert_eq!(
            InitializeProtocolInfrastructureV1::decode(
                &InitializeProtocolInfrastructureV2.to_bytes(),
            ),
            Err(Error::InvalidMagic),
        );
        assert_eq!(
            InitializeProtocolInfrastructureV2::decode(
                &InitializeProtocolInfrastructureV1.to_bytes(),
            ),
            Err(Error::InvalidMagic),
        );
    }

    #[test]
    fn profile_v2_width_magic_version_profile_reserved_and_zero_identities_refuse() {
        let bytes = fixture_v2().to_bytes();
        assert_eq!(
            ProtocolInfrastructureProfileV2::decode(&bytes[..223]),
            Err(Error::InvalidLength),
        );
        let mut extended = Vec::from(bytes);
        extended.push(0);
        assert_eq!(
            ProtocolInfrastructureProfileV2::decode(&extended),
            Err(Error::InvalidLength),
        );
        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V2,
                Error::UnsupportedSchema,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V2,
                Error::UnsupportedArtifactProfile,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V2,
                Error::NonCanonicalReservedBytes,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_TAIL_OFFSET_V2,
                Error::NonCanonicalReservedBytes,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2 - 1,
                Error::NonCanonicalReservedBytes,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V2,
                Error::ZeroProgramIdentity,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_REGISTRY_ARTIFACT_OFFSET_V2,
                Error::ZeroArtifactReleaseId,
            ),
            (
                PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_RENT_ARTIFACT_OFFSET_V2,
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
                ProtocolInfrastructureProfileV2::decode(&hostile),
                Err(expected)
            );
        }
    }

    #[test]
    fn succession_bindings_and_predecessors_must_be_distinct() {
        // V1's two aliasing refusals, unchanged on the successor side.
        assert_eq!(
            ProtocolInfrastructureProfileV2::new(
                binding_fixture(1, 5),
                binding_fixture(1, 4),
                artifact_fixture(2),
                artifact_fixture(4),
            ),
            Err(Error::AliasedInfrastructureBinding),
        );
        assert_eq!(
            ProtocolInfrastructureProfileV2::new(
                binding_fixture(1, 5),
                binding_fixture(3, 5),
                artifact_fixture(2),
                artifact_fixture(4),
            ),
            Err(Error::AliasedInfrastructureBinding),
        );
        // The predecessor ids come from a V1 profile that already refused
        // aliasing, so equal ids here can only be a forged or corrupt image.
        assert_eq!(
            ProtocolInfrastructureProfileV2::new(
                binding_fixture(1, 5),
                binding_fixture(3, 4),
                artifact_fixture(2),
                artifact_fixture(2),
            ),
            Err(Error::AliasedInfrastructureBinding),
        );
        // An unmoved binding keeps its predecessor artifact: admitted.
        assert!(
            ProtocolInfrastructureProfileV2::new(
                binding_fixture(1, 5),
                binding_fixture(3, 4),
                artifact_fixture(2),
                artifact_fixture(4),
            )
            .is_ok()
        );
    }
}
