//! Canonical finite result-domain authority shared by Product and Source.
//!
//! One record owns the ordered source-result partition. Source adapters
//! consume this record; they do not define a parallel outcome order.
//! This file preserves the V1 fixed-layout profile; Product Runtime V2 is the
//! data-defined successor and does not inherit V1's provisional width ceiling.

use core::convert::TryFrom;

use crate::{ContentId, Error, Result, array, byte, content_id, put, require_zero};

/// Mathematical minimum: one ordinary region and one distinct failure outcome.
pub const MIN_RESULT_OUTCOMES: usize = 2;
/// Provisional V1 fixed-layout maximum, already lifted by Product Runtime V2.
pub const MAX_RESULT_OUTCOMES: usize = 16;
/// Maximum ordinary regions in this fixed-layout release.
pub const MAX_RESULT_REGIONS: usize = MAX_RESULT_OUTCOMES - 1;
/// Maximum interior cuts between ordinary regions.
pub const MAX_RESULT_CUTS: usize = MAX_RESULT_REGIONS - 1;
/// Exact calculated byte width of [`FiniteResultDomainV1`].
///
/// The layout is a 16-byte header, three 32-byte IDs, one `u64`
/// denominator, one `u8` region count plus seven reserved alignment bytes,
/// and fourteen `i128` cut numerators. Derived selectors consume no rent.
pub const FINITE_RESULT_DOMAIN_BYTES: usize = 352;
/// Canonical result-domain magic.
pub const FINITE_RESULT_DOMAIN_MAGIC: [u8; 8] = *b"DCLTRDV1";
/// Implemented result-domain schema version.
pub const FINITE_RESULT_DOMAIN_SCHEMA_VERSION: u16 = 1;
/// Canonical finalized-record schema label for [`FiniteResultDomainV1`].
pub const FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/product-finite-result-domain-v1";
/// SHA-256 identity of [`FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x37, 0x3d, 0x8d, 0xf3, 0x60, 0x73, 0xe8, 0x45, 0x54, 0xed, 0xa9, 0x89, 0x11, 0xb8, 0x3a, 0x9c,
    0x13, 0xcb, 0x07, 0x74, 0x54, 0x8f, 0x68, 0x0c, 0xba, 0x66, 0x29, 0x13, 0xdd, 0x66, 0x0e, 0x14,
];
/// Canonical content-identity byte domain for a finite result-domain preimage.
///
/// Hash derivation is an adapter concern; this Product-owned value names the
/// semantic namespace that adapters must supply when identifying this record.
pub const FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1: &[u8] = b"dclutch.result-domain.v1";
/// Closed semantic release for identity-ordered regions plus final failure.
pub const FINITE_RESULT_DOMAIN_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/product-finite-result-domain-release/v1";
/// SHA-256 identity of [`FINITE_RESULT_DOMAIN_RELEASE_PREIMAGE_V1`].
pub const FINITE_RESULT_DOMAIN_RELEASE_ID_V1: [u8; 32] = [
    0x1a, 0xa4, 0x1f, 0x18, 0xfa, 0x8d, 0xee, 0xe0, 0x9d, 0xa1, 0xa1, 0x32, 0x60, 0x65, 0xa9, 0x90,
    0xca, 0x97, 0x1a, 0x0f, 0xc5, 0x9b, 0x77, 0x33, 0xc8, 0x7b, 0xc3, 0x8c, 0xb0, 0x92, 0x53, 0xf7,
];

const COORDINATE_DOMAIN_ID_OFFSET: usize = 16;
const RESULT_UNIT_ID_OFFSET: usize = 48;
const RELEASE_ID_OFFSET: usize = 80;
const DENOMINATOR_OFFSET: usize = 112;
const REGION_COUNT_OFFSET: usize = 120;
const CUTS_OFFSET: usize = 128;

/// Product-owned partition of the exact rational source-result line.
///
/// With ordered cut numerators `c[0..R-1]` and common positive denominator
/// `d`, ordinary region zero is `x < c[0]/d`, each interior region is
/// `c[i-1]/d <= x < c[i]/d`, and the last is `x >= c[R-2]/d`.
///
/// V1 has exactly `N = R + 1` native outcomes. Ordinary region `i` selects
/// outcome `i`; the explicit failure outcome is `R`. Outcome count and all
/// selectors are derived from `R`, so failure cannot alias an ordinary region
/// and no redundant persisted fact can become a second authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FiniteResultDomainV1 {
    coordinate_domain_id: ContentId,
    result_unit_id: ContentId,
    release_id: ContentId,
    denominator: u64,
    cuts: [i128; MAX_RESULT_CUTS],
    region_count: u8,
}

impl FiniteResultDomainV1 {
    /// Construct the one V1 canonical ordering from active cut numerators.
    pub fn new(
        coordinate_domain_id: ContentId,
        result_unit_id: ContentId,
        denominator: u64,
        active_cuts: &[i128],
    ) -> Result<Self> {
        if denominator == 0 {
            return Err(Error::ZeroResultDenominator);
        }
        let region_count = active_cuts
            .len()
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if !(1..=MAX_RESULT_REGIONS).contains(&region_count) {
            return Err(Error::InvalidResultDomain);
        }
        let mut cuts = [0; MAX_RESULT_CUTS];
        let mut previous = None;
        for (index, cut) in active_cuts.iter().copied().enumerate() {
            if previous.is_some_and(|prior| cut <= prior) {
                return Err(Error::InvalidResultDomain);
            }
            *cuts.get_mut(index).ok_or(Error::InvalidResultDomain)? = cut;
            previous = Some(cut);
        }
        Ok(Self {
            coordinate_domain_id,
            result_unit_id,
            release_id: ContentId::new(FINITE_RESULT_DOMAIN_RELEASE_ID_V1)?,
            denominator,
            cuts,
            region_count: u8::try_from(region_count).map_err(|_| Error::ArithmeticOverflow)?,
        })
    }

    /// Decode and independently validate one exact hostile record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FINITE_RESULT_DOMAIN_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != FINITE_RESULT_DOMAIN_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != FINITE_RESULT_DOMAIN_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 121, 7)?;
        let region_count = usize::from(byte(bytes, REGION_COUNT_OFFSET)?);
        if region_count == 0 || region_count > MAX_RESULT_REGIONS {
            return Err(Error::InvalidResultDomain);
        }
        let active_cut_count = region_count.saturating_sub(1);
        let mut cuts = [0; MAX_RESULT_CUTS];
        let mut previous = None;
        for (index, cut) in cuts.iter_mut().enumerate() {
            let offset = index
                .checked_mul(16)
                .and_then(|relative| CUTS_OFFSET.checked_add(relative))
                .ok_or(Error::ArithmeticOverflow)?;
            *cut = i128::from_le_bytes(array(bytes, offset)?);
            if index < active_cut_count {
                if previous.is_some_and(|prior| *cut <= prior) {
                    return Err(Error::InvalidResultDomain);
                }
                previous = Some(*cut);
            } else if *cut != 0 {
                return Err(Error::NonCanonicalReservedBytes);
            }
        }
        let value = Self {
            coordinate_domain_id: content_id(bytes, COORDINATE_DOMAIN_ID_OFFSET)?,
            result_unit_id: content_id(bytes, RESULT_UNIT_ID_OFFSET)?,
            release_id: content_id(bytes, RELEASE_ID_OFFSET)?,
            denominator: u64::from_le_bytes(array(bytes, DENOMINATOR_OFFSET)?),
            cuts,
            region_count: u8::try_from(region_count).map_err(|_| Error::ArithmeticOverflow)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the one exact canonical content preimage.
    pub fn to_bytes(self) -> [u8; FINITE_RESULT_DOMAIN_BYTES] {
        let mut output = [0; FINITE_RESULT_DOMAIN_BYTES];
        put(&mut output, 0, &FINITE_RESULT_DOMAIN_MAGIC);
        put(
            &mut output,
            8,
            &FINITE_RESULT_DOMAIN_SCHEMA_VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            COORDINATE_DOMAIN_ID_OFFSET,
            self.coordinate_domain_id.as_bytes(),
        );
        put(
            &mut output,
            RESULT_UNIT_ID_OFFSET,
            self.result_unit_id.as_bytes(),
        );
        put(&mut output, RELEASE_ID_OFFSET, self.release_id.as_bytes());
        put(
            &mut output,
            DENOMINATOR_OFFSET,
            &self.denominator.to_le_bytes(),
        );
        put(&mut output, REGION_COUNT_OFFSET, &[self.region_count]);
        for (index, cut) in self.cuts.iter().enumerate() {
            let offset = CUTS_OFFSET.saturating_add(index.saturating_mul(16));
            put(&mut output, offset, &cut.to_le_bytes());
        }
        output
    }

    /// Recheck the closed release and canonical V1 shape.
    pub fn validate(self) -> Result<()> {
        if self.release_id.to_bytes() != FINITE_RESULT_DOMAIN_RELEASE_ID_V1
            || self.denominator == 0
            || self.region_count == 0
            || usize::from(self.region_count) > MAX_RESULT_REGIONS
        {
            return Err(Error::InvalidResultDomain);
        }
        let active_cuts = usize::from(self.region_count.saturating_sub(1));
        let mut previous = None;
        for (index, cut) in self.cuts.iter().copied().enumerate() {
            if index < active_cuts {
                if previous.is_some_and(|prior| cut <= prior) {
                    return Err(Error::InvalidResultDomain);
                }
                previous = Some(cut);
            } else if cut != 0 {
                return Err(Error::NonCanonicalReservedBytes);
            }
        }
        Ok(())
    }

    /// Map an exact rational source result to its ordinary Product selector.
    pub fn map(self, numerator: i128, denominator: u64) -> Result<u8> {
        self.validate()?;
        if denominator == 0 {
            return Err(Error::ZeroResultDenominator);
        }
        let mut region = 0usize;
        let cut_count = usize::from(self.region_count.saturating_sub(1));
        while region < cut_count {
            let cut = *self.cuts.get(region).ok_or(Error::InvalidResultDomain)?;
            if rational_less(numerator, denominator, cut, self.denominator)? {
                break;
            }
            region = region.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        u8::try_from(region).map_err(|_| Error::InvalidResultDomain)
    }

    /// Return the source coordinate-domain semantics identity.
    pub const fn coordinate_domain_id(self) -> ContentId {
        self.coordinate_domain_id
    }

    /// Return the exact result/statistic unit identity.
    pub const fn result_unit_id(self) -> ContentId {
        self.result_unit_id
    }

    /// Return the closed mapping semantic release identity.
    pub const fn release_id(self) -> ContentId {
        self.release_id
    }

    /// Return the positive common denominator for all cut numerators.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Borrow the active ordered cut numerators.
    pub fn cuts(&self) -> &[i128] {
        let count = usize::from(self.region_count.saturating_sub(1));
        self.cuts.get(..count).unwrap_or(&[])
    }

    /// Return the number of exhaustive ordinary source-result regions.
    pub const fn region_count(self) -> u8 {
        self.region_count
    }

    /// Return the total native categorical outcome width including failure.
    pub const fn outcome_count(self) -> u8 {
        self.region_count.saturating_add(1)
    }

    /// Return the explicit outcome reserved for resolution failure.
    pub const fn failure_selector(self) -> u8 {
        self.region_count
    }

    /// Return the derived ordinary selector for one active result region.
    pub fn selector(self, region: u8) -> Result<u8> {
        if region >= self.region_count {
            return Err(Error::InvalidFiniteSelector);
        }
        Ok(region)
    }
}

fn rational_less(
    left_numerator: i128,
    left_denominator: u64,
    right_numerator: i128,
    right_denominator: u64,
) -> Result<bool> {
    if left_denominator == 0 || right_denominator == 0 {
        return Err(Error::ZeroResultDenominator);
    }
    match (left_numerator.is_negative(), right_numerator.is_negative()) {
        (true, false) => Ok(true),
        (false, true) => Ok(false),
        (false, false) => Ok(nonnegative_fraction_less(
            left_numerator.unsigned_abs(),
            u128::from(left_denominator),
            right_numerator.unsigned_abs(),
            u128::from(right_denominator),
        )),
        (true, true) => Ok(nonnegative_fraction_less(
            right_numerator.unsigned_abs(),
            u128::from(right_denominator),
            left_numerator.unsigned_abs(),
            u128::from(left_denominator),
        )),
    }
}

fn nonnegative_fraction_less(
    mut left_numerator: u128,
    mut left_denominator: u128,
    mut right_numerator: u128,
    mut right_denominator: u128,
) -> bool {
    let mut inverted = false;
    loop {
        let left_integer = left_numerator / left_denominator;
        let right_integer = right_numerator / right_denominator;
        if left_integer != right_integer {
            return if inverted {
                left_integer > right_integer
            } else {
                left_integer < right_integer
            };
        }
        let left_remainder = left_numerator % left_denominator;
        let right_remainder = right_numerator % right_denominator;
        match (left_remainder == 0, right_remainder == 0) {
            (true, true) => return false,
            (true, false) => return !inverted,
            (false, true) => return inverted,
            (false, false) => {
                left_numerator = left_denominator;
                left_denominator = left_remainder;
                right_numerator = right_denominator;
                right_denominator = right_remainder;
                inverted = !inverted;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id;

    #[test]
    fn exact_domain_round_trips_and_maps_edges_upward() {
        let domain =
            FiniteResultDomainV1::new(id(1), id(2), 10, &[-10, 0, 25]).expect("canonical domain");
        assert_eq!(domain.region_count(), 4);
        assert_eq!(domain.outcome_count(), 5);
        assert_eq!(domain.failure_selector(), 4);
        assert_eq!(domain.map(-11, 10), Ok(0));
        assert_eq!(domain.map(-10, 10), Ok(1));
        assert_eq!(domain.map(25, 10), Ok(3));
        assert_eq!(FiniteResultDomainV1::decode(&domain.to_bytes()), Ok(domain));
    }

    #[test]
    fn failure_and_selectors_are_derived_without_wire_aliases() {
        let domain = FiniteResultDomainV1::new(id(1), id(2), 1, &[0]).expect("canonical domain");
        assert_eq!(domain.selector(0), Ok(0));
        assert_eq!(domain.selector(1), Ok(1));
        assert_eq!(domain.failure_selector(), 2);
        assert_eq!(domain.selector(2), Err(Error::InvalidFiniteSelector));
        let mut invalid_count = domain.to_bytes();
        invalid_count[REGION_COUNT_OFFSET] = 16;
        assert_eq!(
            FiniteResultDomainV1::decode(&invalid_count),
            Err(Error::InvalidResultDomain)
        );
    }

    #[test]
    fn inactive_cuts_and_reserved_bytes_are_canonical_zero() {
        let domain = FiniteResultDomainV1::new(id(1), id(2), 1, &[0]).expect("canonical domain");
        let mut dirty_cut = domain.to_bytes();
        dirty_cut[CUTS_OFFSET + 16] = 1;
        assert_eq!(
            FiniteResultDomainV1::decode(&dirty_cut),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut dirty_reserved = domain.to_bytes();
        dirty_reserved[121] = 1;
        assert_eq!(
            FiniteResultDomainV1::decode(&dirty_reserved),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut trailing = domain.to_bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            FiniteResultDomainV1::decode(&trailing),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn same_width_substitution_changes_authority_bytes() {
        let left = FiniteResultDomainV1::new(id(1), id(2), 1, &[0]).expect("left domain");
        let right = FiniteResultDomainV1::new(id(1), id(2), 1, &[1]).expect("right domain");
        assert_eq!(left.outcome_count(), right.outcome_count());
        assert_ne!(left.to_bytes(), right.to_bytes());
    }

    #[test]
    fn exact_comparison_is_total_at_signed_extremes() {
        let domain =
            FiniteResultDomainV1::new(id(1), id(2), u64::MAX, &[-1, 0, 1]).expect("extreme domain");
        assert_eq!(domain.map(i128::MIN, 1), Ok(0));
        assert_eq!(domain.map(-1, u64::MAX), Ok(1));
        assert_eq!(domain.map(0, u64::MAX), Ok(2));
        assert_eq!(domain.map(i128::MAX, 1), Ok(3));
        assert_eq!(
            rational_less(i128::MAX, u64::MAX, i128::MAX - 1, u64::MAX),
            Ok(false)
        );
        assert_eq!(
            rational_less(i128::MIN, u64::MAX, i128::MIN + 1, u64::MAX),
            Ok(true)
        );
    }
}
