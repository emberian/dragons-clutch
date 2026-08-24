//! Content preimages for Product terms, occurrences, and instances.

use crate::capacity::{CapacityProfileId, CapacityProfileV1, MIN_PARTITION_CELLS};
use crate::claim::CategoricalUnitV1;
use crate::{ContentId, Error, Result, array, byte, content_id, put, require_zero};

/// Exact byte width of [`TermsV1`].
pub const TERMS_BYTES: usize = 160;
/// Exact byte width of [`OccurrenceV1`].
pub const OCCURRENCE_BYTES: usize = 128;
/// Exact byte width of [`InstanceV1`].
pub const INSTANCE_BYTES: usize = 160;
/// Canonical Terms magic.
pub const TERMS_MAGIC: [u8; 8] = *b"DCLTTRM1";
/// Canonical Occurrence magic.
pub const OCCURRENCE_MAGIC: [u8; 8] = *b"DCLTOCC1";
/// Canonical Product-instance magic.
pub const INSTANCE_MAGIC: [u8; 8] = *b"DCLTINS1";
/// Implemented schema shared by the three Product preimages.
pub const PRODUCT_SCHEMA_VERSION: u16 = 1;

const HEADER_RESERVED_OFFSET: usize = 11;
const HEADER_RESERVED_BYTES: usize = 5;

/// Mandatory state-partition contract for liability-bearing terms.
///
/// The named capacity verifier must establish these four properties over the
/// content-addressed artifact before liabilities mint. This byte is a
/// requirement, not a self-attestation or substitute for verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PartitionRequirement {
    /// Cells are exhaustive, pairwise disjoint, strictly ordered, and encoded
    /// in the verifier release's unique canonical form.
    ExhaustiveDisjointOrderedCanonical = 1,
}

impl PartitionRequirement {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::ExhaustiveDisjointOrderedCanonical),
            _ => Err(Error::UnknownPartitionRequirement),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::ExhaustiveDisjointOrderedCanonical => 1,
        }
    }
}

/// Inputs to one reusable Product terms preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TermsV1Input {
    /// Capacity profile governing artifact verification and bounds.
    pub capacity_profile_id: CapacityProfileId,
    /// Release identity assigning semantics to the Terms artifact.
    pub semantic_release_id: ContentId,
    /// Content identity of the exact Terms/partition artifact.
    pub artifact_id: ContentId,
    /// Evidence identity emitted by the selected bounded partition verifier.
    pub partition_evidence_id: ContentId,
    /// Exact artifact byte length.
    pub artifact_bytes: u32,
    /// Unique minimal canonical page count.
    pub page_count: u32,
    /// Number of cells in canonical order in the state partition.
    pub partition_cell_count: u32,
}

/// Reusable Product semantics and one verified finite state partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TermsV1 {
    capacity_profile_id: CapacityProfileId,
    semantic_release_id: ContentId,
    artifact_id: ContentId,
    partition_evidence_id: ContentId,
    artifact_bytes: u32,
    page_count: u32,
    partition_cell_count: u32,
}

impl TermsV1 {
    /// Construct Terms after checking its size against the supplied profile.
    ///
    /// The composing hash boundary must also establish that `profile` hashes
    /// to `capacity_profile_id`; this SDK-free crate intentionally owns no hash.
    pub fn new(input: TermsV1Input, profile: CapacityProfileV1) -> Result<Self> {
        profile.validate_artifact(input.artifact_bytes, input.page_count)?;
        profile.validate_partition(input.partition_cell_count)?;
        Ok(Self {
            capacity_profile_id: input.capacity_profile_id,
            semantic_release_id: input.semantic_release_id,
            artifact_id: input.artifact_id,
            partition_evidence_id: input.partition_evidence_id,
            artifact_bytes: input.artifact_bytes,
            page_count: input.page_count,
            partition_cell_count: input.partition_cell_count,
        })
    }

    /// Decode a structurally canonical Terms preimage.
    ///
    /// Call [`Self::validate_capacity`] with the authenticated profile record
    /// before admitting liabilities.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != TERMS_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != TERMS_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != PRODUCT_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        PartitionRequirement::decode(byte(bytes, 10)?)?;
        require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
        require_zero(bytes, 156, 4)?;
        let value = Self {
            capacity_profile_id: CapacityProfileId::new(content_id(bytes, 16)?),
            semantic_release_id: content_id(bytes, 48)?,
            artifact_id: content_id(bytes, 80)?,
            partition_evidence_id: content_id(bytes, 112)?,
            artifact_bytes: u32::from_le_bytes(array(bytes, 144)?),
            page_count: u32::from_le_bytes(array(bytes, 148)?),
            partition_cell_count: u32::from_le_bytes(array(bytes, 152)?),
        };
        if value.partition_cell_count < MIN_PARTITION_CELLS {
            return Err(Error::PartitionTooSmall);
        }
        if value.artifact_bytes == 0 || value.page_count == 0 {
            return Err(Error::ZeroCapacity);
        }
        Ok(value)
    }

    /// Encode the exact Terms content preimage.
    pub fn to_bytes(self) -> [u8; TERMS_BYTES] {
        let mut output = [0; TERMS_BYTES];
        put(&mut output, 0, &TERMS_MAGIC);
        put(&mut output, 8, &PRODUCT_SCHEMA_VERSION.to_le_bytes());
        put(
            &mut output,
            10,
            &[PartitionRequirement::ExhaustiveDisjointOrderedCanonical.byte()],
        );
        put(
            &mut output,
            16,
            self.capacity_profile_id.content_id().as_bytes(),
        );
        put(&mut output, 48, self.semantic_release_id.as_bytes());
        put(&mut output, 80, self.artifact_id.as_bytes());
        put(&mut output, 112, self.partition_evidence_id.as_bytes());
        put(&mut output, 144, &self.artifact_bytes.to_le_bytes());
        put(&mut output, 148, &self.page_count.to_le_bytes());
        put(&mut output, 152, &self.partition_cell_count.to_le_bytes());
        output
    }

    /// Recheck decoded Terms against the authenticated capacity record.
    pub fn validate_capacity(self, profile: CapacityProfileV1) -> Result<()> {
        profile.validate_artifact(self.artifact_bytes, self.page_count)?;
        profile.validate_partition(self.partition_cell_count)
    }

    /// Return the selected capacity-profile identity.
    pub const fn capacity_profile_id(self) -> CapacityProfileId {
        self.capacity_profile_id
    }

    /// Return the canonical partition width.
    pub const fn partition_cell_count(self) -> u32 {
        self.partition_cell_count
    }

    /// Return the Terms semantic release identity.
    pub const fn semantic_release_id(self) -> ContentId {
        self.semantic_release_id
    }

    /// Return the exact Terms artifact identity.
    pub const fn artifact_id(self) -> ContentId {
        self.artifact_id
    }

    /// Return the partition-verification evidence identity.
    pub const fn partition_evidence_id(self) -> ContentId {
        self.partition_evidence_id
    }

    /// Return the exact partition artifact byte length.
    pub const fn artifact_bytes(self) -> u32 {
        self.artifact_bytes
    }

    /// Return the unique minimal partition artifact page count.
    pub const fn page_count(self) -> u32 {
        self.page_count
    }
}

/// Inputs to one concrete occurrence under reusable Terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceV1Input {
    /// Content identity of the governing [`TermsV1`] preimage.
    pub terms_id: ContentId,
    /// Capacity profile repeated to make substitution checks local.
    pub capacity_profile_id: CapacityProfileId,
    /// Content identity of occurrence-specific semantic parameters.
    pub occurrence_artifact_id: ContentId,
    /// Exact occurrence artifact byte length.
    pub artifact_bytes: u32,
    /// Unique minimal canonical page count.
    pub page_count: u32,
}

/// One event governed by reusable Product Terms.
///
/// The occurrence artifact may contain event time, location, strike, or other
/// semantic parameters defined by the Terms release. It must not contain RPC,
/// oracle-account, transport, or resolver workflow authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceV1 {
    terms_id: ContentId,
    capacity_profile_id: CapacityProfileId,
    occurrence_artifact_id: ContentId,
    artifact_bytes: u32,
    page_count: u32,
}

impl OccurrenceV1 {
    /// Construct one occurrence within the supplied capacity envelope.
    pub fn new(input: OccurrenceV1Input, profile: CapacityProfileV1) -> Result<Self> {
        profile.validate_artifact(input.artifact_bytes, input.page_count)?;
        Ok(Self {
            terms_id: input.terms_id,
            capacity_profile_id: input.capacity_profile_id,
            occurrence_artifact_id: input.occurrence_artifact_id,
            artifact_bytes: input.artifact_bytes,
            page_count: input.page_count,
        })
    }

    /// Decode one structurally canonical occurrence preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != OCCURRENCE_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != OCCURRENCE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != PRODUCT_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 120, 8)?;
        let value = Self {
            terms_id: content_id(bytes, 16)?,
            capacity_profile_id: CapacityProfileId::new(content_id(bytes, 48)?),
            occurrence_artifact_id: content_id(bytes, 80)?,
            artifact_bytes: u32::from_le_bytes(array(bytes, 112)?),
            page_count: u32::from_le_bytes(array(bytes, 116)?),
        };
        if value.artifact_bytes == 0 || value.page_count == 0 {
            return Err(Error::ZeroCapacity);
        }
        Ok(value)
    }

    /// Encode the exact occurrence content preimage.
    pub fn to_bytes(self) -> [u8; OCCURRENCE_BYTES] {
        let mut output = [0; OCCURRENCE_BYTES];
        put(&mut output, 0, &OCCURRENCE_MAGIC);
        put(&mut output, 8, &PRODUCT_SCHEMA_VERSION.to_le_bytes());
        put(&mut output, 16, self.terms_id.as_bytes());
        put(
            &mut output,
            48,
            self.capacity_profile_id.content_id().as_bytes(),
        );
        put(&mut output, 80, self.occurrence_artifact_id.as_bytes());
        put(&mut output, 112, &self.artifact_bytes.to_le_bytes());
        put(&mut output, 116, &self.page_count.to_le_bytes());
        output
    }

    /// Check occurrence linkage to an authenticated Terms record and ID.
    pub fn validate_terms(self, terms_id: ContentId, terms: TermsV1) -> Result<()> {
        if self.terms_id != terms_id || self.capacity_profile_id != terms.capacity_profile_id() {
            return Err(Error::IdentityMismatch);
        }
        Ok(())
    }

    /// Check this occurrence's artifact against its authenticated profile.
    pub fn validate_capacity(self, profile: CapacityProfileV1) -> Result<()> {
        profile.validate_artifact(self.artifact_bytes, self.page_count)
    }

    /// Return the governing Terms identity.
    pub const fn terms_id(self) -> ContentId {
        self.terms_id
    }

    /// Return the governing capacity profile identity.
    pub const fn capacity_profile_id(self) -> CapacityProfileId {
        self.capacity_profile_id
    }

    /// Return the occurrence-specific semantic artifact identity.
    pub const fn occurrence_artifact_id(self) -> ContentId {
        self.occurrence_artifact_id
    }

    /// Return the exact occurrence artifact byte length.
    pub const fn artifact_bytes(self) -> u32 {
        self.artifact_bytes
    }

    /// Return the unique minimal occurrence artifact page count.
    pub const fn page_count(self) -> u32 {
        self.page_count
    }
}

/// Inputs to one Product instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceV1Input {
    /// Governing Terms content identity.
    pub terms_id: ContentId,
    /// Concrete Occurrence content identity.
    pub occurrence_id: ContentId,
    /// Elementary categorical claim-basis content identity.
    pub claim_basis_id: ContentId,
    /// Capacity-profile identity shared by all linked records.
    pub capacity_profile_id: CapacityProfileId,
    /// Exact partition width repeated for local redemption checks.
    pub partition_cell_count: u32,
}

/// One elementary categorical native-claim family over one occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceV1 {
    terms_id: ContentId,
    occurrence_id: ContentId,
    claim_basis_id: ContentId,
    capacity_profile_id: CapacityProfileId,
    partition_cell_count: u32,
}

impl InstanceV1 {
    /// Construct a Product instance with a nontrivial finite partition.
    pub fn new(input: InstanceV1Input) -> Result<Self> {
        if input.partition_cell_count < MIN_PARTITION_CELLS {
            return Err(Error::PartitionTooSmall);
        }
        Ok(Self {
            terms_id: input.terms_id,
            occurrence_id: input.occurrence_id,
            claim_basis_id: input.claim_basis_id,
            capacity_profile_id: input.capacity_profile_id,
            partition_cell_count: input.partition_cell_count,
        })
    }

    /// Decode one exact Product-instance preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != INSTANCE_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != INSTANCE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != PRODUCT_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 148, 12)?;
        Self::new(InstanceV1Input {
            terms_id: content_id(bytes, 16)?,
            occurrence_id: content_id(bytes, 48)?,
            claim_basis_id: content_id(bytes, 80)?,
            capacity_profile_id: CapacityProfileId::new(content_id(bytes, 112)?),
            partition_cell_count: u32::from_le_bytes(array(bytes, 144)?),
        })
    }

    /// Encode the exact Product-instance content preimage.
    pub fn to_bytes(self) -> [u8; INSTANCE_BYTES] {
        let mut output = [0; INSTANCE_BYTES];
        put(&mut output, 0, &INSTANCE_MAGIC);
        put(&mut output, 8, &PRODUCT_SCHEMA_VERSION.to_le_bytes());
        put(&mut output, 16, self.terms_id.as_bytes());
        put(&mut output, 48, self.occurrence_id.as_bytes());
        put(&mut output, 80, self.claim_basis_id.as_bytes());
        put(
            &mut output,
            112,
            self.capacity_profile_id.content_id().as_bytes(),
        );
        put(&mut output, 144, &self.partition_cell_count.to_le_bytes());
        output
    }

    /// Check all direct Terms and Occurrence links.
    pub fn validate_occurrence(
        self,
        terms_id: ContentId,
        terms: TermsV1,
        occurrence_id: ContentId,
        occurrence: OccurrenceV1,
    ) -> Result<()> {
        if self.terms_id != terms_id
            || self.occurrence_id != occurrence_id
            || self.capacity_profile_id != terms.capacity_profile_id()
            || self.partition_cell_count != terms.partition_cell_count()
        {
            return Err(Error::IdentityMismatch);
        }
        occurrence.validate_terms(terms_id, terms)?;
        if occurrence.capacity_profile_id() != self.capacity_profile_id {
            return Err(Error::IdentityMismatch);
        }
        Ok(())
    }

    /// Check the bound claim-basis record and its authenticated content ID.
    pub fn validate_claim_basis(
        self,
        claim_basis_id: ContentId,
        claim_basis: CategoricalUnitV1,
    ) -> Result<()> {
        if self.claim_basis_id != claim_basis_id
            || self.capacity_profile_id != claim_basis.capacity_profile_id()
            || self.partition_cell_count != claim_basis.partition_cell_count()
        {
            return Err(Error::IdentityMismatch);
        }
        Ok(())
    }

    /// Return the bound Product occurrence identity.
    pub const fn occurrence_id(self) -> ContentId {
        self.occurrence_id
    }

    /// Return the governing Terms identity.
    pub const fn terms_id(self) -> ContentId {
        self.terms_id
    }

    /// Return the bound claim-basis identity.
    pub const fn claim_basis_id(self) -> ContentId {
        self.claim_basis_id
    }

    /// Return the shared capacity-profile identity.
    pub const fn capacity_profile_id(self) -> CapacityProfileId {
        self.capacity_profile_id
    }

    /// Return the exact finite partition width.
    pub const fn partition_cell_count(self) -> u32 {
        self.partition_cell_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capacity::{CapacityEnvelope, CapacityProfileV1Input};
    use crate::claim::{CategoricalUnitV1, CategoricalUnitV1Input};
    use crate::id;

    fn capacity() -> (CapacityProfileId, CapacityProfileV1) {
        let profile = CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Measured,
            verifier_release_id: id(1),
            envelope_basis_id: id(2),
            max_artifact_bytes: 256,
            page_payload_bytes: 64,
            max_pages: 4,
            max_partition_cells: 16,
        })
        .expect("valid profile");
        (CapacityProfileId::new(id(3)), profile)
    }

    fn terms() -> TermsV1 {
        let (profile_id, profile) = capacity();
        TermsV1::new(
            TermsV1Input {
                capacity_profile_id: profile_id,
                semantic_release_id: id(4),
                artifact_id: id(5),
                partition_evidence_id: id(6),
                artifact_bytes: 128,
                page_count: 2,
                partition_cell_count: 4,
            },
            profile,
        )
        .expect("valid terms")
    }

    #[test]
    fn exact_product_preimages_round_trip() {
        let terms = terms();
        let terms_bytes = terms.to_bytes();
        assert_eq!(terms_bytes.len(), TERMS_BYTES);
        assert_eq!(terms_bytes.get(10), Some(&1));
        assert_eq!(TermsV1::decode(&terms_bytes), Ok(terms));

        let (capacity_id, profile) = capacity();
        let occurrence = OccurrenceV1::new(
            OccurrenceV1Input {
                terms_id: id(7),
                capacity_profile_id: capacity_id,
                occurrence_artifact_id: id(8),
                artifact_bytes: 64,
                page_count: 1,
            },
            profile,
        )
        .expect("valid occurrence");
        assert_eq!(OccurrenceV1::decode(&occurrence.to_bytes()), Ok(occurrence));

        let instance = InstanceV1::new(InstanceV1Input {
            terms_id: id(7),
            occurrence_id: id(9),
            claim_basis_id: id(10),
            capacity_profile_id: capacity_id,
            partition_cell_count: 4,
        })
        .expect("valid instance");
        assert_eq!(InstanceV1::decode(&instance.to_bytes()), Ok(instance));
    }

    #[test]
    fn refuses_weakened_partition_and_zero_identity() {
        let mut bytes = terms().to_bytes();
        bytes[10] = 0;
        assert_eq!(
            TermsV1::decode(&bytes),
            Err(Error::UnknownPartitionRequirement)
        );

        let mut bytes = terms().to_bytes();
        bytes[80..112].fill(0);
        assert_eq!(TermsV1::decode(&bytes), Err(Error::ZeroIdentifier));

        let mut bytes = terms().to_bytes();
        bytes[152..156].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(TermsV1::decode(&bytes), Err(Error::PartitionTooSmall));
    }

    #[test]
    fn linked_records_refuse_substitution() {
        let (capacity_id, profile) = capacity();
        let terms = terms();
        let occurrence = OccurrenceV1::new(
            OccurrenceV1Input {
                terms_id: id(7),
                capacity_profile_id: capacity_id,
                occurrence_artifact_id: id(8),
                artifact_bytes: 64,
                page_count: 1,
            },
            profile,
        )
        .expect("valid occurrence");
        assert_eq!(
            occurrence.validate_terms(id(99), terms),
            Err(Error::IdentityMismatch)
        );

        let claim_basis = CategoricalUnitV1::new(
            CategoricalUnitV1Input {
                capacity_profile_id: capacity_id,
                outcome_count: 4,
            },
            profile,
        )
        .expect("claim basis");
        let instance = InstanceV1::new(InstanceV1Input {
            terms_id: id(7),
            occurrence_id: id(9),
            claim_basis_id: id(10),
            capacity_profile_id: capacity_id,
            partition_cell_count: 4,
        })
        .expect("instance");
        assert_eq!(instance.validate_claim_basis(id(10), claim_basis), Ok(()));
        assert_eq!(
            instance.validate_claim_basis(id(11), claim_basis),
            Err(Error::IdentityMismatch)
        );

        let wrong_width = CategoricalUnitV1::new(
            CategoricalUnitV1Input {
                capacity_profile_id: capacity_id,
                outcome_count: 3,
            },
            profile,
        )
        .expect("claim basis");
        assert_eq!(
            instance.validate_claim_basis(id(10), wrong_width),
            Err(Error::IdentityMismatch)
        );
    }
}
