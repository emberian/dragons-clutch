//! Immutable, content-addressed artifact and partition capacity profiles.

use crate::{
    ContentId, Error, Result, array, byte, canonical_pages, content_id, put, require_zero,
};

/// Exact byte width of [`CapacityProfileV1`].
pub const CAPACITY_PROFILE_BYTES: usize = 96;
/// Canonical Product capacity-profile magic, distinct from capability manifests.
pub const CAPACITY_PROFILE_MAGIC: [u8; 8] = *b"DCLTPCP1";
/// Implemented capacity-profile schema version.
pub const CAPACITY_PROFILE_SCHEMA_VERSION: u16 = 1;

/// Mathematical lower bound for a liability-bearing state partition.
///
/// A one-cell partition cannot express mutually exclusive claims. This bound
/// is mathematical/Product-semantic, not an SVM measurement.
pub const MIN_PARTITION_CELLS: u32 = 2;

const ENVELOPE_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const HEADER_RESERVED_BYTES: usize = 5;
const VERIFIER_RELEASE_ID_OFFSET: usize = 16;
const ENVELOPE_BASIS_ID_OFFSET: usize = 48;
const MAX_ARTIFACT_BYTES_OFFSET: usize = 80;
const PAGE_PAYLOAD_BYTES_OFFSET: usize = 84;
const MAX_PAGES_OFFSET: usize = 88;
const MAX_PARTITION_CELLS_OFFSET: usize = 92;

/// Nature of the evidence behind a fixed capacity envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CapacityEnvelope {
    /// Bound supported by measurements named by `envelope_basis_id`.
    Measured = 1,
    /// Temporary bound whose named basis is a lifting plan, not measurement.
    Provisional = 2,
}

impl CapacityEnvelope {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Measured),
            2 => Ok(Self::Provisional),
            _ => Err(Error::UnknownEnvelopeKind),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Measured => 1,
            Self::Provisional => 2,
        }
    }
}

/// Typed identity of a capacity-profile content preimage.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct CapacityProfileId(ContentId);

impl CapacityProfileId {
    /// Construct a nonzero capacity-profile content identity.
    pub const fn new(content_id: ContentId) -> Self {
        Self(content_id)
    }

    /// Decode an exact nonzero capacity-profile identity.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(Self(ContentId::decode(bytes)?))
    }

    /// Return the underlying opaque content identity.
    pub const fn content_id(self) -> ContentId {
        self.0
    }
}

/// Inputs to one immutable capacity profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityProfileV1Input {
    /// Whether the envelope is measured or provisional.
    pub envelope: CapacityEnvelope,
    /// Release identity of the bounded artifact/partition verifier.
    pub verifier_release_id: ContentId,
    /// Measurement-manifest ID or, for provisional bounds, lifting-plan ID.
    pub envelope_basis_id: ContentId,
    /// Maximum bytes in any one content-addressed Product artifact.
    pub max_artifact_bytes: u32,
    /// Exact maximum payload bytes in each canonical artifact page.
    pub page_payload_bytes: u32,
    /// Unique minimal page count covering `max_artifact_bytes`.
    pub max_pages: u32,
    /// Maximum cells in an exhaustive canonical state partition.
    pub max_partition_cells: u32,
}

/// Immutable bounds and verifier semantics for Product artifacts and partitions.
///
/// All size/count fields are explicitly measured or provisional as selected by
/// [`CapacityEnvelope`]. A provisional profile names its lifting plan in
/// `envelope_basis_id`; a measured profile names its measurement manifest.
/// Coefficient widths/counts do not exist here: user portfolio recipes are
/// exact-width contract records, not native liability or paged artifact bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityProfileV1 {
    envelope: CapacityEnvelope,
    verifier_release_id: ContentId,
    envelope_basis_id: ContentId,
    max_artifact_bytes: u32,
    page_payload_bytes: u32,
    max_pages: u32,
    max_partition_cells: u32,
}

impl CapacityProfileV1 {
    /// Validate and construct one capacity profile.
    pub fn new(input: CapacityProfileV1Input) -> Result<Self> {
        if input.max_artifact_bytes == 0 || input.page_payload_bytes == 0 || input.max_pages == 0 {
            return Err(Error::ZeroCapacity);
        }
        if input.max_partition_cells < MIN_PARTITION_CELLS {
            return Err(Error::PartitionTooSmall);
        }
        if canonical_pages(input.max_artifact_bytes, input.page_payload_bytes)? != input.max_pages {
            return Err(Error::NonCanonicalPaging);
        }
        Ok(Self {
            envelope: input.envelope,
            verifier_release_id: input.verifier_release_id,
            envelope_basis_id: input.envelope_basis_id,
            max_artifact_bytes: input.max_artifact_bytes,
            page_payload_bytes: input.page_payload_bytes,
            max_pages: input.max_pages,
            max_partition_cells: input.max_partition_cells,
        })
    }

    /// Decode one exact canonical capacity profile.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CAPACITY_PROFILE_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != CAPACITY_PROFILE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != CAPACITY_PROFILE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
        Self::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::decode(byte(bytes, ENVELOPE_OFFSET)?)?,
            verifier_release_id: content_id(bytes, VERIFIER_RELEASE_ID_OFFSET)?,
            envelope_basis_id: content_id(bytes, ENVELOPE_BASIS_ID_OFFSET)?,
            max_artifact_bytes: u32::from_le_bytes(array(bytes, MAX_ARTIFACT_BYTES_OFFSET)?),
            page_payload_bytes: u32::from_le_bytes(array(bytes, PAGE_PAYLOAD_BYTES_OFFSET)?),
            max_pages: u32::from_le_bytes(array(bytes, MAX_PAGES_OFFSET)?),
            max_partition_cells: u32::from_le_bytes(array(bytes, MAX_PARTITION_CELLS_OFFSET)?),
        })
    }

    /// Return the exact content preimage for external content addressing.
    pub fn to_bytes(self) -> [u8; CAPACITY_PROFILE_BYTES] {
        let mut output = [0; CAPACITY_PROFILE_BYTES];
        put(&mut output, 0, &CAPACITY_PROFILE_MAGIC);
        put(
            &mut output,
            8,
            &CAPACITY_PROFILE_SCHEMA_VERSION.to_le_bytes(),
        );
        put(&mut output, ENVELOPE_OFFSET, &[self.envelope.byte()]);
        put(
            &mut output,
            VERIFIER_RELEASE_ID_OFFSET,
            self.verifier_release_id.as_bytes(),
        );
        put(
            &mut output,
            ENVELOPE_BASIS_ID_OFFSET,
            self.envelope_basis_id.as_bytes(),
        );
        put(
            &mut output,
            MAX_ARTIFACT_BYTES_OFFSET,
            &self.max_artifact_bytes.to_le_bytes(),
        );
        put(
            &mut output,
            PAGE_PAYLOAD_BYTES_OFFSET,
            &self.page_payload_bytes.to_le_bytes(),
        );
        put(&mut output, MAX_PAGES_OFFSET, &self.max_pages.to_le_bytes());
        put(
            &mut output,
            MAX_PARTITION_CELLS_OFFSET,
            &self.max_partition_cells.to_le_bytes(),
        );
        output
    }

    /// Check one artifact's exact byte and paging declaration.
    pub fn validate_artifact(self, artifact_bytes: u32, page_count: u32) -> Result<()> {
        if artifact_bytes == 0 {
            return Err(Error::ZeroCapacity);
        }
        if artifact_bytes > self.max_artifact_bytes || page_count > self.max_pages {
            return Err(Error::ArtifactExceedsCapacity);
        }
        if canonical_pages(artifact_bytes, self.page_payload_bytes)? != page_count {
            return Err(Error::PageCountMismatch);
        }
        Ok(())
    }

    /// Check a state-partition width against the profile envelope.
    pub fn validate_partition(self, partition_cells: u32) -> Result<()> {
        if partition_cells < MIN_PARTITION_CELLS {
            return Err(Error::PartitionTooSmall);
        }
        if partition_cells > self.max_partition_cells {
            return Err(Error::PartitionExceedsCapacity);
        }
        Ok(())
    }

    /// Return whether this is a measured or provisional envelope.
    pub const fn envelope(self) -> CapacityEnvelope {
        self.envelope
    }

    /// Return the bounded verifier release identity.
    pub const fn verifier_release_id(self) -> ContentId {
        self.verifier_release_id
    }

    /// Return the measurement-manifest or lifting-plan identity.
    pub const fn envelope_basis_id(self) -> ContentId {
        self.envelope_basis_id
    }

    /// Return the maximum artifact bytes.
    pub const fn max_artifact_bytes(self) -> u32 {
        self.max_artifact_bytes
    }

    /// Return exact canonical payload bytes per page.
    pub const fn page_payload_bytes(self) -> u32 {
        self.page_payload_bytes
    }

    /// Return the maximum canonical page count.
    pub const fn max_pages(self) -> u32 {
        self.max_pages
    }

    /// Return the maximum partition cells.
    pub const fn max_partition_cells(self) -> u32 {
        self.max_partition_cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id;

    fn input() -> CapacityProfileV1Input {
        CapacityProfileV1Input {
            envelope: CapacityEnvelope::Provisional,
            verifier_release_id: id(1),
            envelope_basis_id: id(2),
            max_artifact_bytes: 320,
            page_payload_bytes: 96,
            max_pages: 4,
            max_partition_cells: 16,
        }
    }

    fn profile() -> CapacityProfileV1 {
        CapacityProfileV1::new(input()).expect("valid profile")
    }

    #[test]
    fn exact_round_trip_and_offsets() {
        let value = profile();
        let bytes = value.to_bytes();
        assert_eq!(CAPACITY_PROFILE_BYTES, 96);
        assert_eq!(CAPACITY_PROFILE_MAGIC, *b"DCLTPCP1");
        assert_eq!(bytes.get(0..8), Some(CAPACITY_PROFILE_MAGIC.as_slice()));
        assert_eq!(bytes.get(10), Some(&2));
        assert_eq!(bytes.get(80..84), Some(320u32.to_le_bytes().as_slice()));
        assert_eq!(bytes.get(92..96), Some(16u32.to_le_bytes().as_slice()));
        assert_eq!(CapacityProfileV1::decode(&bytes), Ok(value));
    }

    #[test]
    fn hostile_lengths_headers_identifiers_and_reserved_bytes_refuse() {
        let bytes = profile().to_bytes();
        for length in 0..CAPACITY_PROFILE_BYTES {
            assert_eq!(
                CapacityProfileV1::decode(bytes.get(..length).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut trailing = [0u8; CAPACITY_PROFILE_BYTES + 1];
        trailing[..CAPACITY_PROFILE_BYTES].copy_from_slice(&bytes);
        assert_eq!(
            CapacityProfileV1::decode(&trailing),
            Err(Error::InvalidLength)
        );

        let mut changed = bytes;
        changed[0] = 0;
        assert_eq!(
            CapacityProfileV1::decode(&changed),
            Err(Error::InvalidMagic)
        );
        let mut changed = bytes;
        changed[8] = 2;
        assert_eq!(
            CapacityProfileV1::decode(&changed),
            Err(Error::UnsupportedSchema)
        );
        let mut changed = bytes;
        changed[10] = 3;
        assert_eq!(
            CapacityProfileV1::decode(&changed),
            Err(Error::UnknownEnvelopeKind)
        );
        let mut changed = bytes;
        changed[11] = 1;
        assert_eq!(
            CapacityProfileV1::decode(&changed),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut changed = bytes;
        changed[16..48].fill(0);
        assert_eq!(
            CapacityProfileV1::decode(&changed),
            Err(Error::ZeroIdentifier)
        );
    }

    #[test]
    fn construction_and_use_refuse_noncanonical_bounds() {
        let mut changed = input();
        changed.max_artifact_bytes = 0;
        assert_eq!(CapacityProfileV1::new(changed), Err(Error::ZeroCapacity));
        let mut changed = input();
        changed.max_partition_cells = 1;
        assert_eq!(
            CapacityProfileV1::new(changed),
            Err(Error::PartitionTooSmall)
        );
        let mut changed = input();
        changed.max_pages = 3;
        assert_eq!(
            CapacityProfileV1::new(changed),
            Err(Error::NonCanonicalPaging)
        );

        let value = profile();
        assert_eq!(value.validate_artifact(0, 1), Err(Error::ZeroCapacity));
        assert_eq!(
            value.validate_artifact(321, 4),
            Err(Error::ArtifactExceedsCapacity)
        );
        assert_eq!(
            value.validate_artifact(192, 1),
            Err(Error::PageCountMismatch)
        );
        assert_eq!(value.validate_artifact(192, 2), Ok(()));
        assert_eq!(value.validate_partition(1), Err(Error::PartitionTooSmall));
        assert_eq!(
            value.validate_partition(17),
            Err(Error::PartitionExceedsCapacity)
        );
        assert_eq!(value.validate_partition(16), Ok(()));
    }
}
