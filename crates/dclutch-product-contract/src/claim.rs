//! Elementary categorical-unit native claim basis.

use crate::capacity::{CapacityProfileId, CapacityProfileV1, MIN_PARTITION_CELLS};
use crate::{Error, Result, array, content_id, put, require_zero};

/// Exact byte width of the only V1 native claim basis.
pub const CATEGORICAL_UNIT_BYTES: usize = 56;
/// Canonical categorical-unit basis magic.
pub const CATEGORICAL_UNIT_MAGIC: [u8; 8] = *b"DCLTCBU1";
/// Implemented native claim-basis schema version.
pub const CLAIM_BASIS_SCHEMA_VERSION: u16 = 1;
/// Canonical finalized-record schema label for [`CategoricalUnitV1`].
pub const CATEGORICAL_CLAIM_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/categorical-unit-claim-v1";
/// SHA-256 identity of [`CATEGORICAL_CLAIM_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0xd3, 0x86, 0x0a, 0x71, 0x29, 0x03, 0x33, 0xab, 0x8d, 0x15, 0x43, 0x11, 0x75, 0x29, 0x30, 0x4f,
    0x1d, 0xcd, 0x2a, 0xe1, 0x42, 0xff, 0xdb, 0xd6, 0x0c, 0xc7, 0x15, 0xb8, 0x62, 0x58, 0xcc, 0x6d,
];

const CAPACITY_PROFILE_ID_OFFSET: usize = 16;
const OUTCOME_COUNT_OFFSET: usize = 48;

/// Input for the elementary categorical-unit native basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalUnitV1Input {
    /// Capacity profile bounding the state partition.
    pub capacity_profile_id: CapacityProfileId,
    /// Number of exhaustive, disjoint, canonically ordered partition cells.
    pub outcome_count: u32,
}

/// The only Product V1 native liability basis.
///
/// There is exactly one native claim per exhaustive ordered partition cell.
/// Claim `i` pays one collateral atom iff terminal cell `i` occurs and zero
/// otherwise. Portfolios, rational recipes, evaluator identities, polynomial
/// coefficients, and rounding policy are deliberately not native claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalUnitV1 {
    capacity_profile_id: CapacityProfileId,
    outcome_count: u32,
}

impl CategoricalUnitV1 {
    /// Construct the elementary basis within an authenticated capacity envelope.
    pub fn new(input: CategoricalUnitV1Input, profile: CapacityProfileV1) -> Result<Self> {
        profile.validate_partition(input.outcome_count)?;
        Ok(Self {
            capacity_profile_id: input.capacity_profile_id,
            outcome_count: input.outcome_count,
        })
    }

    /// Decode one exact categorical-unit basis.
    ///
    /// Call [`Self::validate_capacity`] with the authenticated profile before
    /// admitting liabilities.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CATEGORICAL_UNIT_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != CATEGORICAL_UNIT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != CLAIM_BASIS_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 52, 4)?;
        let outcome_count = u32::from_le_bytes(array(bytes, OUTCOME_COUNT_OFFSET)?);
        if outcome_count < MIN_PARTITION_CELLS {
            return Err(Error::PartitionTooSmall);
        }
        Ok(Self {
            capacity_profile_id: CapacityProfileId::new(content_id(
                bytes,
                CAPACITY_PROFILE_ID_OFFSET,
            )?),
            outcome_count,
        })
    }

    /// Encode the exact native-basis content preimage.
    pub fn to_bytes(self) -> [u8; CATEGORICAL_UNIT_BYTES] {
        let mut output = [0; CATEGORICAL_UNIT_BYTES];
        put(&mut output, 0, &CATEGORICAL_UNIT_MAGIC);
        put(&mut output, 8, &CLAIM_BASIS_SCHEMA_VERSION.to_le_bytes());
        put(
            &mut output,
            CAPACITY_PROFILE_ID_OFFSET,
            self.capacity_profile_id.content_id().as_bytes(),
        );
        put(
            &mut output,
            OUTCOME_COUNT_OFFSET,
            &self.outcome_count.to_le_bytes(),
        );
        output
    }

    /// Return the capacity-profile identity.
    pub const fn capacity_profile_id(self) -> CapacityProfileId {
        self.capacity_profile_id
    }

    /// Return the exhaustive partition/native-claim width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Return the exhaustive partition/native-claim width.
    pub const fn partition_cell_count(self) -> u32 {
        self.outcome_count
    }

    /// Validate the decoded categorical width against an authenticated profile.
    pub fn validate_capacity(self, profile: CapacityProfileV1) -> Result<()> {
        profile.validate_partition(self.outcome_count)
    }
}

#[cfg(test)]
mod tests {
    use crate::capacity::{CapacityEnvelope, CapacityProfileV1Input};
    use crate::id;

    use super::*;

    fn profile(max_partition_cells: u32) -> (CapacityProfileId, CapacityProfileV1) {
        let value = CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Provisional,
            verifier_release_id: id(1),
            envelope_basis_id: id(2),
            max_artifact_bytes: 320,
            page_payload_bytes: 96,
            max_pages: 4,
            max_partition_cells,
        })
        .expect("valid capacity");
        (CapacityProfileId::new(id(3)), value)
    }

    fn basis() -> CategoricalUnitV1 {
        let (capacity_profile_id, value) = profile(16);
        CategoricalUnitV1::new(
            CategoricalUnitV1Input {
                capacity_profile_id,
                outcome_count: 4,
            },
            value,
        )
        .expect("valid basis")
    }

    #[test]
    fn exact_layout_round_trips_without_evaluator_or_rounding_authority() {
        let value = basis();
        let bytes = value.to_bytes();
        assert_eq!(CATEGORICAL_UNIT_BYTES, 56);
        assert_eq!(bytes.get(0..8), Some(CATEGORICAL_UNIT_MAGIC.as_slice()));
        assert_eq!(bytes.get(10..16), Some([0; 6].as_slice()));
        assert_eq!(bytes.get(48..52), Some(4u32.to_le_bytes().as_slice()));
        assert_eq!(CategoricalUnitV1::decode(&bytes), Ok(value));
    }

    #[test]
    fn hostile_lengths_headers_reserved_and_counts_refuse() {
        let bytes = basis().to_bytes();
        for length in 0..CATEGORICAL_UNIT_BYTES {
            assert_eq!(
                CategoricalUnitV1::decode(bytes.get(..length).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut trailing = [0u8; CATEGORICAL_UNIT_BYTES + 1];
        trailing[..CATEGORICAL_UNIT_BYTES].copy_from_slice(&bytes);
        assert_eq!(
            CategoricalUnitV1::decode(&trailing),
            Err(Error::InvalidLength)
        );

        let mut changed = bytes;
        changed[0] = 0;
        assert_eq!(
            CategoricalUnitV1::decode(&changed),
            Err(Error::InvalidMagic)
        );
        let mut changed = bytes;
        changed[8] = 2;
        assert_eq!(
            CategoricalUnitV1::decode(&changed),
            Err(Error::UnsupportedSchema)
        );
        let mut changed = bytes;
        changed[10] = 1;
        assert_eq!(
            CategoricalUnitV1::decode(&changed),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut changed = bytes;
        changed[16..48].fill(0);
        assert_eq!(
            CategoricalUnitV1::decode(&changed),
            Err(Error::ZeroIdentifier)
        );
        let mut changed = bytes;
        changed[48..52].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(
            CategoricalUnitV1::decode(&changed),
            Err(Error::PartitionTooSmall)
        );
    }

    #[test]
    fn construction_and_authenticated_capacity_refuse_bad_widths() {
        let (capacity_profile_id, capacity) = profile(16);
        assert_eq!(
            CategoricalUnitV1::new(
                CategoricalUnitV1Input {
                    capacity_profile_id,
                    outcome_count: 1,
                },
                capacity,
            ),
            Err(Error::PartitionTooSmall)
        );
        assert_eq!(
            CategoricalUnitV1::new(
                CategoricalUnitV1Input {
                    capacity_profile_id,
                    outcome_count: 17,
                },
                capacity,
            ),
            Err(Error::PartitionExceedsCapacity)
        );
        assert_eq!(
            basis().validate_capacity(profile(3).1),
            Err(Error::PartitionExceedsCapacity)
        );
    }
}
