//! Typed categorical-unit and finite exact claim-basis profiles.

use crate::capacity::{CapacityProfileId, CapacityProfileV1, MIN_PARTITION_CELLS};
use crate::{ContentId, Error, Result, array, byte, content_id, put, require_zero};

/// Exact byte width of the categorical-unit fast-path profile.
pub const CATEGORICAL_UNIT_BYTES: usize = 64;
/// Exact byte width of a paged finite-exact payout profile.
pub const FINITE_EXACT_BYTES: usize = 208;
/// Canonical categorical-unit profile magic.
pub const CATEGORICAL_UNIT_MAGIC: [u8; 8] = *b"DCLTCBU1";
/// Canonical finite-exact profile magic.
pub const FINITE_EXACT_MAGIC: [u8; 8] = *b"DCLTFIN1";
/// Implemented claim-basis schema version.
pub const CLAIM_BASIS_SCHEMA_VERSION: u16 = 1;
/// Mathematical categorical-unit payout denominator.
pub const CATEGORICAL_UNIT_DENOMINATOR: u64 = 1;

const CATEGORICAL_KIND: u8 = 1;
const FINITE_EXACT_KIND: u8 = 2;

/// One named rounding boundary for collateral redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RedemptionRounding {
    /// Refuse a redemption whose exact numerator is not divisible.
    ExactOnly = 0,
    /// Pay the quotient rounded toward zero at the redemption boundary.
    FloorAtRedemption = 1,
    /// Pay the quotient and persist the remainder under the named fractional
    /// credit policy; this mode requires a nonzero policy identity.
    CreditRemainder = 2,
}

impl RedemptionRounding {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::ExactOnly),
            1 => Ok(Self::FloorAtRedemption),
            2 => Ok(Self::CreditRemainder),
            _ => Err(Error::UnknownRoundingMode),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::ExactOnly => 0,
            Self::FloorAtRedemption => 1,
            Self::CreditRemainder => 2,
        }
    }
}

/// Maximum polynomial coefficient degree admitted by this finite evaluator
/// contract. This is a program-profile bound, not an SVM account-size bound.
/// New entry/page envelopes do not change it; higher degrees require a new
/// evaluator/schema release rather than silently changing existing semantics.
pub const MAX_COEFFICIENT_DEGREE: u8 = 3;

/// Exact polynomial coefficient degree admitted by the finite profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CoefficientDegree {
    /// Piecewise constant payout over the ordered partition.
    Zero = 0,
    /// Degree-one exact coefficient family.
    One = 1,
    /// Degree-two exact coefficient family.
    Two = 2,
    /// Degree-three exact coefficient family.
    Three = 3,
}

impl CoefficientDegree {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Zero),
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            _ => Err(Error::UnsupportedCoefficientDegree),
        }
    }

    /// Return the exact number of coefficients per partition cell.
    pub const fn coefficient_count(self) -> u32 {
        match self {
            Self::Zero => 1,
            Self::One => 2,
            Self::Two => 3,
            Self::Three => 4,
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Zero => 0,
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }
}

/// Input for the compact categorical-unit claim basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalUnitV1Input {
    /// Capacity profile bounding the state partition.
    pub capacity_profile_id: CapacityProfileId,
    /// Number of exhaustive, disjoint, canonically ordered outcome claims.
    pub outcome_count: u32,
    /// Must be one: a winning claim atom pays one collateral atom.
    pub payout_denominator: u64,
    /// Must be [`RedemptionRounding::ExactOnly`].
    pub rounding: RedemptionRounding,
}

/// Compact categorical-unit claim basis.
///
/// Its evaluator is this schema itself: outcome `i` pays one collateral atom
/// iff the finite terminal selector is `i`, otherwise zero. This retains the
/// cheap categorical path without making it the universal Product ontology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalUnitV1 {
    capacity_profile_id: CapacityProfileId,
    outcome_count: u32,
}

impl CategoricalUnitV1 {
    /// Construct a categorical-unit basis within a capacity envelope.
    pub fn new(input: CategoricalUnitV1Input, profile: CapacityProfileV1) -> Result<Self> {
        profile.validate_partition(input.outcome_count)?;
        if input.payout_denominator != CATEGORICAL_UNIT_DENOMINATOR {
            return Err(Error::InvalidPayoutDenominator);
        }
        if input.rounding != RedemptionRounding::ExactOnly {
            return Err(Error::UnsupportedProfileCombination);
        }
        Ok(Self {
            capacity_profile_id: input.capacity_profile_id,
            outcome_count: input.outcome_count,
        })
    }

    /// Decode one exact categorical-unit profile.
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
        if byte(bytes, 10)? != CATEGORICAL_KIND {
            return Err(Error::UnknownClaimBasisKind);
        }
        if RedemptionRounding::decode(byte(bytes, 11)?)? != RedemptionRounding::ExactOnly {
            return Err(Error::UnsupportedProfileCombination);
        }
        require_zero(bytes, 12, 4)?;
        require_zero(bytes, 52, 4)?;
        let outcome_count = u32::from_le_bytes(array(bytes, 48)?);
        if outcome_count < MIN_PARTITION_CELLS {
            return Err(Error::PartitionTooSmall);
        }
        if u64::from_le_bytes(array(bytes, 56)?) != CATEGORICAL_UNIT_DENOMINATOR {
            return Err(Error::InvalidPayoutDenominator);
        }
        Ok(Self {
            capacity_profile_id: CapacityProfileId::new(content_id(bytes, 16)?),
            outcome_count,
        })
    }

    /// Encode the exact categorical-unit content preimage.
    pub fn to_bytes(self) -> [u8; CATEGORICAL_UNIT_BYTES] {
        let mut output = [0; CATEGORICAL_UNIT_BYTES];
        put(&mut output, 0, &CATEGORICAL_UNIT_MAGIC);
        put(&mut output, 8, &CLAIM_BASIS_SCHEMA_VERSION.to_le_bytes());
        put(&mut output, 10, &[CATEGORICAL_KIND]);
        put(&mut output, 11, &[RedemptionRounding::ExactOnly.byte()]);
        put(
            &mut output,
            16,
            self.capacity_profile_id.content_id().as_bytes(),
        );
        put(&mut output, 48, &self.outcome_count.to_le_bytes());
        put(&mut output, 56, &CATEGORICAL_UNIT_DENOMINATOR.to_le_bytes());
        output
    }

    /// Return the capacity-profile identity.
    pub const fn capacity_profile_id(self) -> CapacityProfileId {
        self.capacity_profile_id
    }

    /// Return the finite outcome count.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Validate a decoded categorical width against its authenticated profile.
    pub fn validate_capacity(self, profile: CapacityProfileV1) -> Result<()> {
        profile.validate_partition(self.outcome_count)
    }
}

/// Inputs for a finite exact payout profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FiniteExactV1Input {
    /// Capacity profile bounding partition and coefficient artifact.
    pub capacity_profile_id: CapacityProfileId,
    /// Content identity of cell-major, degree-ascending exact coefficients.
    pub payout_artifact_id: ContentId,
    /// Release identity of the bounded exact payout evaluator.
    pub evaluator_release_id: ContentId,
    /// Profile identity assigning exact meaning/range to coefficient words.
    pub coefficient_profile_id: ContentId,
    /// Required only for [`RedemptionRounding::CreditRemainder`].
    pub fractional_credit_policy_id: Option<ContentId>,
    /// Positive common denominator for all exact payout coefficients.
    pub payout_denominator: u64,
    /// Exact degree, from zero through three.
    pub coefficient_degree: CoefficientDegree,
    /// Named rounding boundary for collateral redemption.
    pub rounding: RedemptionRounding,
    /// State-partition width, whose canonical order owns artifact cell order.
    pub partition_cell_count: u32,
    /// Must equal `partition_cell_count * (degree + 1)`.
    pub coefficient_entry_count: u32,
    /// Must equal entry count times the capacity profile's word width.
    pub artifact_bytes: u32,
    /// Unique minimal canonical artifact page count.
    pub page_count: u32,
}

/// Content-addressed finite exact payout basis.
///
/// Coefficients are ordered first by the Terms partition's canonical cell
/// order, then by ascending degree within each cell. The evaluator and
/// coefficient-profile identities define exact variable/range semantics. This
/// record is deliberately not bytecode and this crate implements no VM.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FiniteExactV1 {
    capacity_profile_id: CapacityProfileId,
    payout_artifact_id: ContentId,
    evaluator_release_id: ContentId,
    coefficient_profile_id: ContentId,
    fractional_credit_policy_id: Option<ContentId>,
    payout_denominator: u64,
    coefficient_degree: CoefficientDegree,
    rounding: RedemptionRounding,
    partition_cell_count: u32,
    coefficient_entry_count: u32,
    artifact_bytes: u32,
    page_count: u32,
}

impl FiniteExactV1 {
    /// Construct and validate one finite exact payout basis.
    pub fn new(input: FiniteExactV1Input, profile: CapacityProfileV1) -> Result<Self> {
        profile.validate_partition(input.partition_cell_count)?;
        if input.payout_denominator == 0 {
            return Err(Error::InvalidPayoutDenominator);
        }
        let expected_entries = input
            .partition_cell_count
            .checked_mul(input.coefficient_degree.coefficient_count())
            .ok_or(Error::ArithmeticOverflow)?;
        if input.coefficient_entry_count != expected_entries {
            return Err(Error::NonCanonicalCoefficientCount);
        }
        profile.validate_coefficients(
            input.coefficient_entry_count,
            input.artifact_bytes,
            input.page_count,
        )?;
        validate_rounding(
            input.payout_denominator,
            input.rounding,
            input.fractional_credit_policy_id,
        )?;
        Ok(Self {
            capacity_profile_id: input.capacity_profile_id,
            payout_artifact_id: input.payout_artifact_id,
            evaluator_release_id: input.evaluator_release_id,
            coefficient_profile_id: input.coefficient_profile_id,
            fractional_credit_policy_id: input.fractional_credit_policy_id,
            payout_denominator: input.payout_denominator,
            coefficient_degree: input.coefficient_degree,
            rounding: input.rounding,
            partition_cell_count: input.partition_cell_count,
            coefficient_entry_count: input.coefficient_entry_count,
            artifact_bytes: input.artifact_bytes,
            page_count: input.page_count,
        })
    }

    /// Decode one structurally canonical finite exact profile.
    ///
    /// Call [`Self::validate_capacity`] with the authenticated capacity record
    /// before minting liabilities or evaluating payouts.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FINITE_EXACT_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != FINITE_EXACT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != CLAIM_BASIS_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if byte(bytes, 10)? != FINITE_EXACT_KIND {
            return Err(Error::UnknownClaimBasisKind);
        }
        let rounding = RedemptionRounding::decode(byte(bytes, 11)?)?;
        let coefficient_degree = CoefficientDegree::decode(byte(bytes, 12)?)?;
        require_zero(bytes, 13, 3)?;
        require_zero(bytes, 200, 8)?;
        let fractional_bytes = array::<32>(bytes, 144)?;
        let fractional_credit_policy_id = if fractional_bytes.iter().all(|value| *value == 0) {
            None
        } else {
            Some(ContentId::new(fractional_bytes)?)
        };
        let payout_denominator = u64::from_le_bytes(array(bytes, 176)?);
        let partition_cell_count = u32::from_le_bytes(array(bytes, 184)?);
        if partition_cell_count < MIN_PARTITION_CELLS {
            return Err(Error::PartitionTooSmall);
        }
        let coefficient_entry_count = u32::from_le_bytes(array(bytes, 188)?);
        let expected_entries = partition_cell_count
            .checked_mul(coefficient_degree.coefficient_count())
            .ok_or(Error::ArithmeticOverflow)?;
        if coefficient_entry_count != expected_entries {
            return Err(Error::NonCanonicalCoefficientCount);
        }
        validate_rounding(payout_denominator, rounding, fractional_credit_policy_id)?;
        let artifact_bytes = u32::from_le_bytes(array(bytes, 192)?);
        let page_count = u32::from_le_bytes(array(bytes, 196)?);
        if artifact_bytes == 0 || page_count == 0 {
            return Err(Error::ZeroCapacity);
        }
        Ok(Self {
            capacity_profile_id: CapacityProfileId::new(content_id(bytes, 16)?),
            payout_artifact_id: content_id(bytes, 48)?,
            evaluator_release_id: content_id(bytes, 80)?,
            coefficient_profile_id: content_id(bytes, 112)?,
            fractional_credit_policy_id,
            payout_denominator,
            coefficient_degree,
            rounding,
            partition_cell_count,
            coefficient_entry_count,
            artifact_bytes,
            page_count,
        })
    }

    /// Encode the exact finite exact content preimage.
    pub fn to_bytes(self) -> [u8; FINITE_EXACT_BYTES] {
        let mut output = [0; FINITE_EXACT_BYTES];
        put(&mut output, 0, &FINITE_EXACT_MAGIC);
        put(&mut output, 8, &CLAIM_BASIS_SCHEMA_VERSION.to_le_bytes());
        put(&mut output, 10, &[FINITE_EXACT_KIND]);
        put(&mut output, 11, &[self.rounding.byte()]);
        put(&mut output, 12, &[self.coefficient_degree.byte()]);
        put(
            &mut output,
            16,
            self.capacity_profile_id.content_id().as_bytes(),
        );
        put(&mut output, 48, self.payout_artifact_id.as_bytes());
        put(&mut output, 80, self.evaluator_release_id.as_bytes());
        put(&mut output, 112, self.coefficient_profile_id.as_bytes());
        if let Some(policy_id) = self.fractional_credit_policy_id {
            put(&mut output, 144, policy_id.as_bytes());
        }
        put(&mut output, 176, &self.payout_denominator.to_le_bytes());
        put(&mut output, 184, &self.partition_cell_count.to_le_bytes());
        put(
            &mut output,
            188,
            &self.coefficient_entry_count.to_le_bytes(),
        );
        put(&mut output, 192, &self.artifact_bytes.to_le_bytes());
        put(&mut output, 196, &self.page_count.to_le_bytes());
        output
    }

    /// Validate decoded size fields against the authenticated capacity profile.
    pub fn validate_capacity(self, profile: CapacityProfileV1) -> Result<()> {
        profile.validate_partition(self.partition_cell_count)?;
        profile.validate_coefficients(
            self.coefficient_entry_count,
            self.artifact_bytes,
            self.page_count,
        )
    }

    /// Return the selected capacity-profile identity.
    pub const fn capacity_profile_id(self) -> CapacityProfileId {
        self.capacity_profile_id
    }

    /// Return the exact finite partition width.
    pub const fn partition_cell_count(self) -> u32 {
        self.partition_cell_count
    }

    /// Return the positive common payout denominator.
    pub const fn payout_denominator(self) -> u64 {
        self.payout_denominator
    }

    /// Return the named redemption rounding boundary.
    pub const fn rounding(self) -> RedemptionRounding {
        self.rounding
    }

    /// Return an optional fractional-credit policy identity.
    pub const fn fractional_credit_policy_id(self) -> Option<ContentId> {
        self.fractional_credit_policy_id
    }
}

/// Either supported V1 claim-basis profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimBasisProfileV1 {
    /// Cheap exact categorical-unit profile.
    CategoricalUnit(CategoricalUnitV1),
    /// Paged finite exact coefficient profile.
    FiniteExact(FiniteExactV1),
}

impl ClaimBasisProfileV1 {
    /// Decode either exact V1 profile by its unique canonical length and tag.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        match bytes.len() {
            CATEGORICAL_UNIT_BYTES => Ok(Self::CategoricalUnit(CategoricalUnitV1::decode(bytes)?)),
            FINITE_EXACT_BYTES => Ok(Self::FiniteExact(FiniteExactV1::decode(bytes)?)),
            _ => Err(Error::InvalidLength),
        }
    }

    /// Return the selected capacity-profile identity.
    pub const fn capacity_profile_id(self) -> CapacityProfileId {
        match self {
            Self::CategoricalUnit(value) => value.capacity_profile_id(),
            Self::FiniteExact(value) => value.capacity_profile_id(),
        }
    }

    /// Return the finite partition width.
    pub const fn partition_cell_count(self) -> u32 {
        match self {
            Self::CategoricalUnit(value) => value.outcome_count(),
            Self::FiniteExact(value) => value.partition_cell_count(),
        }
    }
}

fn validate_rounding(
    denominator: u64,
    rounding: RedemptionRounding,
    fractional_credit_policy_id: Option<ContentId>,
) -> Result<()> {
    if denominator == 0 {
        return Err(Error::InvalidPayoutDenominator);
    }
    if denominator == 1 && rounding != RedemptionRounding::ExactOnly {
        return Err(Error::UnsupportedProfileCombination);
    }
    match (rounding, fractional_credit_policy_id) {
        (RedemptionRounding::CreditRemainder, Some(_)) => Ok(()),
        (RedemptionRounding::CreditRemainder, None) => Err(Error::UnsupportedProfileCombination),
        (RedemptionRounding::ExactOnly | RedemptionRounding::FloorAtRedemption, None) => Ok(()),
        (RedemptionRounding::ExactOnly | RedemptionRounding::FloorAtRedemption, Some(_)) => {
            Err(Error::UnsupportedProfileCombination)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capacity::{CapacityEnvelope, CapacityProfileV1Input, ExactWordWidth};
    use crate::id;

    fn capacity() -> (CapacityProfileId, CapacityProfileV1) {
        let value = CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Measured,
            word_width: ExactWordWidth::Eight,
            verifier_release_id: id(1),
            envelope_basis_id: id(2),
            max_artifact_bytes: 512,
            page_payload_bytes: 128,
            max_pages: 4,
            max_partition_cells: 16,
            max_coefficient_entries: 64,
        })
        .expect("valid profile");
        (CapacityProfileId::new(id(3)), value)
    }

    fn categorical() -> CategoricalUnitV1 {
        let (profile_id, profile) = capacity();
        CategoricalUnitV1::new(
            CategoricalUnitV1Input {
                capacity_profile_id: profile_id,
                outcome_count: 4,
                payout_denominator: 1,
                rounding: RedemptionRounding::ExactOnly,
            },
            profile,
        )
        .expect("valid categorical basis")
    }

    fn finite(rounding: RedemptionRounding, policy: Option<ContentId>) -> Result<FiniteExactV1> {
        let (profile_id, profile) = capacity();
        FiniteExactV1::new(
            FiniteExactV1Input {
                capacity_profile_id: profile_id,
                payout_artifact_id: id(4),
                evaluator_release_id: id(5),
                coefficient_profile_id: id(6),
                fractional_credit_policy_id: policy,
                payout_denominator: 100,
                coefficient_degree: CoefficientDegree::Two,
                rounding,
                partition_cell_count: 4,
                coefficient_entry_count: 12,
                artifact_bytes: 96,
                page_count: 1,
            },
            profile,
        )
    }

    #[test]
    fn categorical_is_exact_compact_and_hostile_decodable() {
        let value = categorical();
        let bytes = value.to_bytes();
        assert_eq!(bytes.len(), CATEGORICAL_UNIT_BYTES);
        assert_eq!(bytes.get(10), Some(&CATEGORICAL_KIND));
        assert_eq!(bytes.get(56..64), Some(1u64.to_le_bytes().as_slice()));
        assert_eq!(CategoricalUnitV1::decode(&bytes), Ok(value));

        let mut wrong_denominator = bytes;
        wrong_denominator[56..64].copy_from_slice(&2u64.to_le_bytes());
        assert_eq!(
            CategoricalUnitV1::decode(&wrong_denominator),
            Err(Error::InvalidPayoutDenominator)
        );

        let mut rounded = bytes;
        rounded[11] = RedemptionRounding::FloorAtRedemption.byte();
        assert_eq!(
            CategoricalUnitV1::decode(&rounded),
            Err(Error::UnsupportedProfileCombination)
        );
    }

    #[test]
    fn finite_profile_has_exact_order_width_and_round_trip() {
        let value =
            finite(RedemptionRounding::FloorAtRedemption, None).expect("valid finite basis");
        let bytes = value.to_bytes();
        assert_eq!(bytes.len(), FINITE_EXACT_BYTES);
        assert_eq!(bytes.get(10), Some(&FINITE_EXACT_KIND));
        assert_eq!(bytes.get(12), Some(&2));
        assert_eq!(bytes.get(188..192), Some(12u32.to_le_bytes().as_slice()));
        assert_eq!(FiniteExactV1::decode(&bytes), Ok(value));
        assert_eq!(
            ClaimBasisProfileV1::decode(&bytes),
            Ok(ClaimBasisProfileV1::FiniteExact(value))
        );
    }

    #[test]
    fn refuses_noncanonical_coefficient_shape() {
        let (profile_id, profile) = capacity();
        let base = FiniteExactV1Input {
            capacity_profile_id: profile_id,
            payout_artifact_id: id(4),
            evaluator_release_id: id(5),
            coefficient_profile_id: id(6),
            fractional_credit_policy_id: None,
            payout_denominator: 100,
            coefficient_degree: CoefficientDegree::Three,
            rounding: RedemptionRounding::ExactOnly,
            partition_cell_count: 4,
            coefficient_entry_count: 15,
            artifact_bytes: 120,
            page_count: 1,
        };
        assert_eq!(
            FiniteExactV1::new(base, profile),
            Err(Error::NonCanonicalCoefficientCount)
        );

        let mismatched_width = FiniteExactV1Input {
            coefficient_entry_count: 16,
            artifact_bytes: 127,
            ..base
        };
        assert_eq!(
            FiniteExactV1::new(mismatched_width, profile),
            Err(Error::ArtifactWidthMismatch)
        );
    }

    #[test]
    fn fractional_policy_and_rounding_are_bijective() {
        assert_eq!(
            finite(RedemptionRounding::CreditRemainder, None),
            Err(Error::UnsupportedProfileCombination)
        );
        assert_eq!(
            finite(RedemptionRounding::ExactOnly, Some(id(9))),
            Err(Error::UnsupportedProfileCombination)
        );
        assert!(finite(RedemptionRounding::CreditRemainder, Some(id(9))).is_ok());

        let (profile_id, profile) = capacity();
        let unit_denominator_rounded = FiniteExactV1Input {
            capacity_profile_id: profile_id,
            payout_artifact_id: id(4),
            evaluator_release_id: id(5),
            coefficient_profile_id: id(6),
            fractional_credit_policy_id: None,
            payout_denominator: 1,
            coefficient_degree: CoefficientDegree::Zero,
            rounding: RedemptionRounding::FloorAtRedemption,
            partition_cell_count: 4,
            coefficient_entry_count: 4,
            artifact_bytes: 32,
            page_count: 1,
        };
        assert_eq!(
            FiniteExactV1::new(unit_denominator_rounded, profile),
            Err(Error::UnsupportedProfileCombination)
        );
    }

    #[test]
    fn decode_refuses_zero_evaluator_and_unsupported_degree() {
        let value = finite(RedemptionRounding::ExactOnly, None).expect("valid basis");
        let mut bytes = value.to_bytes();
        bytes[80..112].fill(0);
        assert_eq!(FiniteExactV1::decode(&bytes), Err(Error::ZeroIdentifier));

        let mut bytes = value.to_bytes();
        bytes[12] = MAX_COEFFICIENT_DEGREE + 1;
        assert_eq!(
            FiniteExactV1::decode(&bytes),
            Err(Error::UnsupportedCoefficientDegree)
        );
    }
}
