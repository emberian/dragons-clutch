//! Exact authority-neutral comparison of two admitted simplex price vectors.
//!
//! This module measures disagreement; it does not decide which input is fair,
//! fresh, manipulable, or authoritative. An adapter may label the inputs as a
//! market quote and a benchmark only after authenticating both semantic
//! owners. The arithmetic remains symmetric under input exchange.

use crate::{Error, ExactSimplexPricesV1, PortfolioDomainV1, Result};

/// Exact rational disagreement between two prices over one contingent domain.
///
/// Each numerator uses [`Self::cross_scale_denominator`] as its denominator.
/// The total-variation numerator is the sum of positive coordinate differences
/// after exact cross-scaling. Equal simplex sums prove that it also equals the
/// sum of negative differences, so it is exactly one half of L1 disagreement
/// without a division or rounding boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactSimplexDisagreementV1 {
    domain: PortfolioDomainV1,
    left_price_scale: u64,
    right_price_scale: u64,
    cross_scale_denominator: u128,
    total_variation_numerator: u128,
    overlap_numerator: u128,
    maximum_coordinate_gap_numerator: u128,
    maximum_coordinate_gap_index: u8,
    left_only_support_mask: u16,
    right_only_support_mask: u16,
    shared_positive_support_mask: u16,
}

impl ExactSimplexDisagreementV1 {
    /// Immutable Market/Terms/outcome domain shared by both prices.
    pub const fn domain(&self) -> PortfolioDomainV1 {
        self.domain
    }

    /// Original exact scale of the left vector.
    pub const fn left_price_scale(&self) -> u64 {
        self.left_price_scale
    }

    /// Original exact scale of the right vector.
    pub const fn right_price_scale(&self) -> u64 {
        self.right_price_scale
    }

    /// Common denominator `left_scale * right_scale` for every metric.
    pub const fn cross_scale_denominator(&self) -> u128 {
        self.cross_scale_denominator
    }

    /// Exact total-variation distance numerator in cross-scale units.
    pub const fn total_variation_numerator(&self) -> u128 {
        self.total_variation_numerator
    }

    /// Exact shared probability-mass numerator, equal to denominator minus
    /// total variation.
    pub const fn overlap_numerator(&self) -> u128 {
        self.overlap_numerator
    }

    /// Largest exact absolute coordinate difference numerator.
    pub const fn maximum_coordinate_gap_numerator(&self) -> u128 {
        self.maximum_coordinate_gap_numerator
    }

    /// Lowest outcome index attaining the largest coordinate difference.
    pub const fn maximum_coordinate_gap_index(&self) -> u8 {
        self.maximum_coordinate_gap_index
    }

    /// Active outcomes priced positively only by the left vector.
    pub const fn left_only_support_mask(&self) -> u16 {
        self.left_only_support_mask
    }

    /// Active outcomes priced positively only by the right vector.
    pub const fn right_only_support_mask(&self) -> u16 {
        self.right_only_support_mask
    }

    /// Active outcomes priced positively by both vectors.
    pub const fn shared_positive_support_mask(&self) -> u16 {
        self.shared_positive_support_mask
    }

    /// Whether both exact rational simplex vectors are coordinatewise equal.
    pub const fn exactly_equal(&self) -> bool {
        self.total_variation_numerator == 0
    }
}

/// Compare two exact simplex prices without choosing either as fair value.
///
/// Different integer scales are compared by exact cross multiplication. The
/// largest possible cross-scale denominator fits `u128` for any two `u64`
/// scales. Positive and negative deviations are accumulated separately and
/// checked equal; each is bounded by the denominator, avoiding the potential
/// overflow of summing the full L1 distance near the `u64` limit.
pub fn compare_exact_simplex_prices_v1(
    left: ExactSimplexPricesV1,
    right: ExactSimplexPricesV1,
) -> Result<ExactSimplexDisagreementV1> {
    if left.domain() != right.domain() {
        return Err(Error::MismatchedPortfolioDomain);
    }

    let left_scale = u128::from(left.price_scale());
    let right_scale = u128::from(right.price_scale());
    let cross_scale_denominator = left_scale
        .checked_mul(right_scale)
        .ok_or(Error::ArithmeticOverflow)?;
    let active = usize::from(left.outcome_count());
    let mut left_excess = 0u128;
    let mut right_excess = 0u128;
    let mut maximum_coordinate_gap_numerator = 0u128;
    let mut maximum_coordinate_gap_index = 0u8;
    let mut left_only_support_mask = 0u16;
    let mut right_only_support_mask = 0u16;
    let mut shared_positive_support_mask = 0u16;

    let mut outcome = 0usize;
    while outcome < active {
        let left_price = left.prices()[outcome];
        let right_price = right.prices()[outcome];
        let left_cross = u128::from(left_price)
            .checked_mul(right_scale)
            .ok_or(Error::ArithmeticOverflow)?;
        let right_cross = u128::from(right_price)
            .checked_mul(left_scale)
            .ok_or(Error::ArithmeticOverflow)?;
        let gap = if left_cross >= right_cross {
            let difference = left_cross - right_cross;
            left_excess = left_excess
                .checked_add(difference)
                .ok_or(Error::ArithmeticOverflow)?;
            difference
        } else {
            let difference = right_cross - left_cross;
            right_excess = right_excess
                .checked_add(difference)
                .ok_or(Error::ArithmeticOverflow)?;
            difference
        };
        if gap > maximum_coordinate_gap_numerator {
            maximum_coordinate_gap_numerator = gap;
            maximum_coordinate_gap_index =
                u8::try_from(outcome).map_err(|_| Error::ArithmeticOverflow)?;
        }

        let shift = u32::try_from(outcome).map_err(|_| Error::ArithmeticOverflow)?;
        let bit = 1u16.checked_shl(shift).ok_or(Error::ArithmeticOverflow)?;
        if left_price != 0 && right_price == 0 {
            left_only_support_mask |= bit;
        } else if left_price == 0 && right_price != 0 {
            right_only_support_mask |= bit;
        } else if left_price != 0 {
            shared_positive_support_mask |= bit;
        }
        outcome += 1;
    }

    if left_excess != right_excess || left_excess > cross_scale_denominator {
        return Err(Error::InvariantViolation);
    }
    let overlap_numerator = cross_scale_denominator
        .checked_sub(left_excess)
        .ok_or(Error::InvariantViolation)?;

    Ok(ExactSimplexDisagreementV1 {
        domain: left.domain(),
        left_price_scale: left.price_scale(),
        right_price_scale: right.price_scale(),
        cross_scale_denominator,
        total_variation_numerator: left_excess,
        overlap_numerator,
        maximum_coordinate_gap_numerator,
        maximum_coordinate_gap_index,
        left_only_support_mask,
        right_only_support_mask,
        shared_positive_support_mask,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_OUTCOMES;

    fn domain(outcome_count: u8) -> PortfolioDomainV1 {
        PortfolioDomainV1::new([1; 32], [2; 32], outcome_count).unwrap()
    }

    fn prices(
        domain: PortfolioDomainV1,
        scale: u64,
        active: &[u64],
    ) -> ExactSimplexPricesV1 {
        let mut cells = [0u64; MAX_OUTCOMES];
        cells[..active.len()].copy_from_slice(active);
        ExactSimplexPricesV1::new(domain, scale, cells).unwrap()
    }

    #[test]
    fn equal_rationals_with_different_scales_have_full_overlap() {
        let domain = domain(2);
        let left = prices(domain, 4, &[1, 3]);
        let right = prices(domain, 100, &[25, 75]);
        let certificate = compare_exact_simplex_prices_v1(left, right).unwrap();

        assert_eq!(certificate.cross_scale_denominator(), 400);
        assert_eq!(certificate.total_variation_numerator(), 0);
        assert_eq!(certificate.overlap_numerator(), 400);
        assert_eq!(certificate.maximum_coordinate_gap_numerator(), 0);
        assert_eq!(certificate.maximum_coordinate_gap_index(), 0);
        assert_eq!(certificate.shared_positive_support_mask(), 0b11);
        assert!(certificate.exactly_equal());
    }

    #[test]
    fn disagreement_is_exact_symmetric_and_uses_lowest_maximum_index() {
        let domain = domain(3);
        let left = prices(domain, 10, &[5, 3, 2]);
        let right = prices(domain, 20, &[2, 8, 10]);
        let left_right = compare_exact_simplex_prices_v1(left, right).unwrap();
        let right_left = compare_exact_simplex_prices_v1(right, left).unwrap();

        assert_eq!(left_right.cross_scale_denominator(), 200);
        assert_eq!(left_right.total_variation_numerator(), 80);
        assert_eq!(left_right.overlap_numerator(), 120);
        assert_eq!(left_right.maximum_coordinate_gap_numerator(), 80);
        assert_eq!(left_right.maximum_coordinate_gap_index(), 0);
        assert_eq!(
            left_right.total_variation_numerator(),
            right_left.total_variation_numerator()
        );
        assert_eq!(
            left_right.overlap_numerator(),
            right_left.overlap_numerator()
        );
        assert_eq!(
            left_right.maximum_coordinate_gap_numerator(),
            right_left.maximum_coordinate_gap_numerator()
        );
    }

    #[test]
    fn disjoint_extreme_support_fits_full_u64_scales_without_l1_overflow() {
        let domain = domain(2);
        let left = prices(domain, u64::MAX, &[u64::MAX, 0]);
        let right = prices(domain, u64::MAX, &[0, u64::MAX]);
        let certificate = compare_exact_simplex_prices_v1(left, right).unwrap();
        let denominator = u128::from(u64::MAX)
            .checked_mul(u128::from(u64::MAX))
            .unwrap();

        assert_eq!(certificate.cross_scale_denominator(), denominator);
        assert_eq!(certificate.total_variation_numerator(), denominator);
        assert_eq!(certificate.overlap_numerator(), 0);
        assert_eq!(certificate.maximum_coordinate_gap_numerator(), denominator);
        assert_eq!(certificate.maximum_coordinate_gap_index(), 0);
        assert_eq!(certificate.left_only_support_mask(), 0b01);
        assert_eq!(certificate.right_only_support_mask(), 0b10);
        assert_eq!(certificate.shared_positive_support_mask(), 0);
    }

    #[test]
    fn support_holes_and_domain_substitution_are_explicit() {
        let domain = domain(3);
        let left = prices(domain, 10, &[5, 0, 5]);
        let right = prices(domain, 10, &[0, 4, 6]);
        let certificate = compare_exact_simplex_prices_v1(left, right).unwrap();

        assert_eq!(certificate.left_only_support_mask(), 0b001);
        assert_eq!(certificate.right_only_support_mask(), 0b010);
        assert_eq!(certificate.shared_positive_support_mask(), 0b100);

        let foreign = prices(
            PortfolioDomainV1::new([3; 32], [2; 32], 3).unwrap(),
            10,
            &[5, 0, 5],
        );
        assert_eq!(
            compare_exact_simplex_prices_v1(left, foreign),
            Err(Error::MismatchedPortfolioDomain)
        );
    }
}
