//! Protocol-owned semantic identities for source-revision-pinned program roles.
//!
//! Trading and Resolution already have narrower semantic owners and are not in
//! this closure. The five roles here have no independently versioned semantic
//! contract, so their release identity commits the exact checked source
//! revision through one fixed, total preimage owned here. Hashing that preimage
//! remains an adapter concern.

/// Git source-revision width admitted by the release preimage.
pub const SOURCE_REVISION_HEX_BYTES_V1: usize = 40;

/// Fixed domain for a source-revision-pinned role semantic release.
pub const SOURCE_SEMANTIC_RELEASE_DOMAIN_V1: &[u8] =
    b"dclutch/release-set/source-semantic-release/v1\0";

/// Exact preimage width for one source-revision-pinned role.
pub const SOURCE_SEMANTIC_RELEASE_PREIMAGE_BYTES_V1: usize =
    SOURCE_SEMANTIC_RELEASE_DOMAIN_V1.len() + 1 + SOURCE_REVISION_HEX_BYTES_V1;

/// The preimage is exactly its three spans: domain, role coordinate, revision.
///
/// `source_semantic_release_preimage_v1` fills the buffer by tiling those three
/// in order, which is total only while this identity holds. It is definitional
/// today; the assert makes it stay definitional, so a later width change that
/// forgot a term is a build failure rather than a silently zero-padded preimage
/// feeding a release identity.
const _: () = assert!(
    SOURCE_SEMANTIC_RELEASE_PREIMAGE_BYTES_V1
        == SOURCE_SEMANTIC_RELEASE_DOMAIN_V1.len() + 1 + SOURCE_REVISION_HEX_BYTES_V1,
);

/// Closed set of program roles whose semantic identity is source-revision-pinned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceSemanticRoleV1 {
    /// Registry program semantics.
    Registry = 0,
    /// Core program semantics.
    Core = 1,
    /// Claims program semantics.
    Claims = 2,
    /// Custody program semantics.
    Custody = 3,
    /// Rent-credit program semantics.
    RentCredit = 4,
}

impl SourceSemanticRoleV1 {
    /// Canonical human-readable role label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::Core => "core",
            Self::Claims => "claims",
            Self::Custody => "custody",
            Self::RentCredit => "rent-credit",
        }
    }

    /// Stable one-byte role coordinate in the release preimage.
    pub const fn coordinate(self) -> u8 {
        self as u8
    }
}

/// Refusal returned while constructing a source semantic-release preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSemanticReleaseErrorV1 {
    /// Source revision was not exactly forty lowercase hexadecimal bytes.
    SourceRevision,
}

/// Construct the exact protocol-owned preimage for one role and source revision.
pub fn source_semantic_release_preimage_v1(
    role: SourceSemanticRoleV1,
    source_revision: &[u8],
) -> core::result::Result<
    [u8; SOURCE_SEMANTIC_RELEASE_PREIMAGE_BYTES_V1],
    SourceSemanticReleaseErrorV1,
> {
    if source_revision.len() != SOURCE_REVISION_HEX_BYTES_V1
        || source_revision
            .iter()
            .any(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        || source_revision.iter().all(|byte| *byte == b'0')
    {
        return Err(SourceSemanticReleaseErrorV1::SourceRevision);
    }
    // The three spans below tile the preimage exactly, and nothing here can
    // refuse: the width is DEFINED as domain + one role coordinate + revision,
    // and the revision's length was just admitted as exactly
    // SOURCE_REVISION_HEX_BYTES_V1. So the panics the previous indexed writes
    // carried were unreachable by construction rather than load-bearing, and
    // there is no refusal for this rewrite to preserve.
    //
    // Writing it as one zip over the concatenated source keeps that totality
    // and removes the indexing. Zip's own failure mode is truncation, which
    // WOULD be a silent short preimage, so the const assert above makes the
    // definition that rules it out a compile-time obligation instead of a
    // comment: change the width without changing its three terms and this stops
    // building, rather than quietly hashing a partly-zero preimage.
    let coordinate = role.coordinate();
    let mut preimage = [0_u8; SOURCE_SEMANTIC_RELEASE_PREIMAGE_BYTES_V1];
    for (slot, byte) in preimage.iter_mut().zip(
        SOURCE_SEMANTIC_RELEASE_DOMAIN_V1
            .iter()
            .chain(core::iter::once(&coordinate))
            .chain(source_revision.iter()),
    ) {
        *slot = *byte;
    }
    Ok(preimage)
}

#[cfg(test)]
// These assertions read the preimage by the same three coordinates the builder
// writes, which is the point: an out-of-bounds read here IS the failure the
// test exists to report, and turning it into a checked `get` would let a short
// preimage pass as `None == None`. The repository's test files carry this same
// allow at file scope; an inline module needs it here.
#[allow(clippy::indexing_slicing, clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    #[test]
    fn role_closure_and_preimage_coordinates_are_exact() {
        let revision = b"0123456789abcdef0123456789abcdef01234567";
        let roles = [
            SourceSemanticRoleV1::Registry,
            SourceSemanticRoleV1::Core,
            SourceSemanticRoleV1::Claims,
            SourceSemanticRoleV1::Custody,
            SourceSemanticRoleV1::RentCredit,
        ];
        for (coordinate, role) in roles.into_iter().enumerate() {
            let preimage = source_semantic_release_preimage_v1(role, revision).expect("preimage");
            assert_eq!(
                &preimage[..SOURCE_SEMANTIC_RELEASE_DOMAIN_V1.len()],
                SOURCE_SEMANTIC_RELEASE_DOMAIN_V1
            );
            assert_eq!(
                preimage[SOURCE_SEMANTIC_RELEASE_DOMAIN_V1.len()],
                coordinate as u8
            );
            assert_eq!(
                &preimage[SOURCE_SEMANTIC_RELEASE_DOMAIN_V1.len() + 1..],
                revision
            );
        }
    }

    #[test]
    fn hostile_source_revisions_refuse() {
        for hostile in [
            &b"0123456789abcdef0123456789abcdef0123456"[..],
            &b"0123456789abcdef0123456789abcdef012345678"[..],
            &b"0123456789ABCDEF0123456789ABCDEF01234567"[..],
            &b"gggggggggggggggggggggggggggggggggggggggg"[..],
            &b"0000000000000000000000000000000000000000"[..],
        ] {
            assert_eq!(
                source_semantic_release_preimage_v1(SourceSemanticRoleV1::Core, hostile),
                Err(SourceSemanticReleaseErrorV1::SourceRevision)
            );
        }
    }

    #[test]
    fn roles_cannot_alias_one_preimage() {
        let revision = b"0123456789abcdef0123456789abcdef01234567";
        let registry =
            source_semantic_release_preimage_v1(SourceSemanticRoleV1::Registry, revision)
                .expect("registry");
        let core = source_semantic_release_preimage_v1(SourceSemanticRoleV1::Core, revision)
            .expect("core");
        assert_ne!(registry, core);
    }
}
