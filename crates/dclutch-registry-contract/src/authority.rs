//! Immutable Market authority-envelope selection.

use dclutch_core_contract::{ContentId, MarketIdentity};

use crate::{
    ActivatedExecutionReleaseSetV1, Error, IDENTITY_BYTES, Result, copy_infallible, put_u16,
    read_array, read_u16, require_zero,
};

/// Exact bytes in one execution-authority manifest.
pub const EXECUTION_AUTHORITY_MANIFEST_BYTES_V1: usize = 80;
/// Canonical execution-authority manifest magic.
pub const EXECUTION_AUTHORITY_MANIFEST_MAGIC_V1: [u8; 8] = *b"DCLTEAM1";
/// Implemented authority-manifest schema.
pub const EXECUTION_AUTHORITY_MANIFEST_SCHEMA_VERSION_V1: u16 = 1;
/// Implemented authority-manifest fixed-layout profile.
pub const EXECUTION_AUTHORITY_MANIFEST_PROFILE_V1: u16 = 1;
/// Schema/validator identity for execution-authority manifests.
///
/// This is SHA-256 of `dclutch/schema/execution-authority-manifest-v1`.
pub const EXECUTION_AUTHORITY_MANIFEST_SCHEMA_ID_V1: [u8; IDENTITY_BYTES] = [
    0xaa, 0x62, 0xcb, 0xc2, 0xaa, 0x4b, 0x09, 0x58, 0x55, 0x91, 0x93, 0x71, 0x85, 0x96, 0x16, 0x0b,
    0x50, 0xc2, 0x2d, 0x0c, 0xdf, 0x23, 0xde, 0xa1, 0x8e, 0x09, 0x40, 0x17, 0xe4, 0x1e, 0xee, 0x6e,
];

const SCHEMA_OFFSET: usize = 8;
const PROFILE_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 12;
const RESERVED_BYTES: usize = 4;
const SEMANTIC_CAPABILITY_MANIFEST_OFFSET: usize = 16;
const EXECUTION_RELEASE_SET_OFFSET: usize = 48;

/// Single immutable envelope selected by a successor Market.
///
/// The envelope preserves the existing semantic capability manifest as its
/// sole capability owner while adding exactly one execution-release-set
/// coordinate.  The Market selects only this envelope's content identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionAuthorityManifestV1 {
    semantic_capability_manifest_id: ContentId,
    execution_release_set_id: ContentId,
}

impl ExecutionAuthorityManifestV1 {
    /// Construct one authority envelope with distinct child identities.
    pub fn new(
        semantic_capability_manifest_id: ContentId,
        execution_release_set_id: ContentId,
    ) -> Result<Self> {
        if semantic_capability_manifest_id == execution_release_set_id {
            return Err(Error::ReleaseSetSelectionMismatch);
        }
        Ok(Self {
            semantic_capability_manifest_id,
            execution_release_set_id,
        })
    }

    /// Hostile-decode one exact canonical authority envelope.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != EXECUTION_AUTHORITY_MANIFEST_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..EXECUTION_AUTHORITY_MANIFEST_MAGIC_V1.len())
            != Some(EXECUTION_AUTHORITY_MANIFEST_MAGIC_V1.as_slice())
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != EXECUTION_AUTHORITY_MANIFEST_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, PROFILE_OFFSET)? != EXECUTION_AUTHORITY_MANIFEST_PROFILE_V1 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        Self::new(
            ContentId::new(read_array(bytes, SEMANTIC_CAPABILITY_MANIFEST_OFFSET)?)
                .map_err(|_| Error::ZeroIdentity)?,
            ContentId::new(read_array(bytes, EXECUTION_RELEASE_SET_OFFSET)?)
                .map_err(|_| Error::ZeroIdentity)?,
        )
    }

    /// Encode the one canonical authority-envelope preimage.
    pub fn to_bytes(self) -> [u8; EXECUTION_AUTHORITY_MANIFEST_BYTES_V1] {
        let mut output = [0; EXECUTION_AUTHORITY_MANIFEST_BYTES_V1];
        copy_infallible(&mut output, 0, &EXECUTION_AUTHORITY_MANIFEST_MAGIC_V1);
        put_u16(
            &mut output,
            SCHEMA_OFFSET,
            EXECUTION_AUTHORITY_MANIFEST_SCHEMA_VERSION_V1,
        );
        put_u16(
            &mut output,
            PROFILE_OFFSET,
            EXECUTION_AUTHORITY_MANIFEST_PROFILE_V1,
        );
        copy_infallible(
            &mut output,
            SEMANTIC_CAPABILITY_MANIFEST_OFFSET,
            self.semantic_capability_manifest_id.as_bytes(),
        );
        copy_infallible(
            &mut output,
            EXECUTION_RELEASE_SET_OFFSET,
            self.execution_release_set_id.as_bytes(),
        );
        output
    }

    /// Return the existing capability-manifest semantic owner.
    pub const fn semantic_capability_manifest_id(self) -> ContentId {
        self.semantic_capability_manifest_id
    }

    /// Return the selected execution-release-set content identity.
    pub const fn execution_release_set_id(self) -> ContentId {
        self.execution_release_set_id
    }
}

/// Ephemeral witness of one authenticated immutable Market selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketExecutionV1 {
    market: MarketIdentity,
    semantic_capability_manifest_id: ContentId,
    execution_release_set_id: ContentId,
}

impl AuthenticatedMarketExecutionV1 {
    /// Return the exact immutable Market identity.
    pub const fn market(self) -> MarketIdentity {
        self.market
    }

    /// Return the selected semantic capability-manifest identity.
    pub const fn semantic_capability_manifest_id(self) -> ContentId {
        self.semantic_capability_manifest_id
    }

    /// Return the selected and activated execution-release-set identity.
    pub const fn execution_release_set_id(self) -> ContentId {
        self.execution_release_set_id
    }
}

/// Authenticate the one successor execution authority selected by a Market.
///
/// `finalized_authority_manifest_id` must be the digest of the exact finalized
/// authority-manifest bytes, established by the composing record adapter.  This
/// function then closes both immutable identity joins without accepting a
/// caller-authored program or release coordinate.
pub fn authenticate_market_execution_v1(
    market: MarketIdentity,
    finalized_authority_manifest_id: ContentId,
    authority_manifest: ExecutionAuthorityManifestV1,
    activated_release_set: ActivatedExecutionReleaseSetV1,
) -> Result<AuthenticatedMarketExecutionV1> {
    if market.capability_manifest_id() != finalized_authority_manifest_id {
        return Err(Error::MarketAuthorityManifestMismatch);
    }
    if authority_manifest.execution_release_set_id()
        != activated_release_set.execution_release_set_id()
    {
        return Err(Error::ReleaseSetSelectionMismatch);
    }
    Ok(AuthenticatedMarketExecutionV1 {
        market,
        semantic_capability_manifest_id: authority_manifest.semantic_capability_manifest_id(),
        execution_release_set_id: authority_manifest.execution_release_set_id(),
    })
}
