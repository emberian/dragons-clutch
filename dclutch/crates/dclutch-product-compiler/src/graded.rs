//! Honest categorical successor for graded payoff recipes.
//!
//! A nontrivial graded native claim basis is impossible over indivisible
//! collateral atoms when every single claim atom must settle independently:
//! if nonnegative integer payouts `p_i(x)` must satisfy
//! `sum_i p_i(x) = 1`, exactly one `p_i(x)` is one and every other payout is
//! zero. The basis is necessarily categorical. Fractional hat weights would
//! require fractional collateral, bundle-dependent remainder assignment, or a
//! minimum settlement lot, each of which changes native liability semantics.
//!
//! This module therefore keeps the existing one-hot categorical basis and
//! projects a graded user payoff to one exact rational payout per ordinary
//! result region, plus one separately chosen failure payout.
//! [`GradedRoundingBoundaryV1::CellMidpoint`] is the sole named approximation
//! boundary: the formula is sampled at the exact rational midpoint of each
//! finite compiler region. This is a categorical approximation, not a native
//! ramp or a pointwise error guarantee. All later arithmetic is checked and exact. The
//! resulting [`ProductShape::OrderedRangeBuckets`] remains a user portfolio
//! over fully collateralized elementary claims; it creates no new liability
//! kind and requires no Market-core state.

use super::{
    CanonicalPartition, CompilationContext, CompileError, CompileRequest, ExactAmount,
    ProductShape, ScaledDomain,
};

/// The only graded-to-categorical rounding/projection boundary in this release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GradedRoundingBoundaryV1 {
    /// Evaluate each cell at its exact rational midpoint.
    ///
    /// A cell `[a,b)` (and the final closed cell) uses `(a+b)/2`. Formula
    /// evaluation retains its exact rational result; it does not round again.
    CellMidpoint,
}

/// A nonnegative payoff formula admitted by the categorical projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GradedShapeV1 {
    /// Zero through `start`, linear to `cap` at `end`, then capped.
    CappedRamp {
        /// First coordinate of the affine ramp.
        start: i128,
        /// Coordinate where the cap is reached.
        end: i128,
        /// Nonnegative exact cap.
        cap: ExactAmount,
    },
    /// Zero through `start`, linear to `cap`, then linear to zero at `end`.
    Tent {
        /// First coordinate of the ascending segment.
        start: i128,
        /// Unique peak coordinate.
        peak: i128,
        /// End coordinate of the descending segment.
        end: i128,
        /// Nonnegative exact peak payout.
        cap: ExactAmount,
    },
    /// Fixed nonnegative payout on `[start,end)` and zero elsewhere.
    RangeBand {
        /// Inclusive lower band coordinate.
        start: i128,
        /// Exclusive upper band coordinate.
        end: i128,
        /// Nonnegative exact in-band payout.
        payout: ExactAmount,
    },
}

/// Exact categorical projection of one graded formula.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedCategoricalShapeV1<const N: usize> {
    partition: CanonicalPartition,
    payouts: [ExactAmount; N],
    failure_payout: ExactAmount,
    boundary: GradedRoundingBoundaryV1,
}

impl<const N: usize> ProjectedCategoricalShapeV1<N> {
    /// Borrow the exhaustive ordered partition receiving the projected payouts.
    pub const fn partition(&self) -> &CanonicalPartition {
        &self.partition
    }

    /// Borrow every exact payout in categorical cell order.
    pub const fn payouts(&self) -> &[ExactAmount; N] {
        &self.payouts
    }

    /// Return the sole named projection boundary.
    pub const fn rounding_boundary(&self) -> GradedRoundingBoundaryV1 {
        self.boundary
    }

    /// Convert the projection into the existing compiler's executable shape.
    ///
    /// The returned shape commits the actual categorical payout vector. The
    /// source formula is explanatory input, never a second payout authority.
    pub fn into_product_shape(self) -> ProductShape {
        let price_regions = self.partition.cuts().len().saturating_add(1);
        ProductShape::OrderedRangeBuckets {
            cut_points: self.partition.cuts().to_vec(),
            payouts: self.payouts.into_iter().take(price_regions).collect(),
            failure_payout: self.failure_payout,
        }
    }

    /// Build a complete compiler request without allowing domain substitution.
    pub fn into_compile_request(self, context: CompilationContext) -> CompileRequest<N> {
        let domain = self.partition.domain();
        CompileRequest {
            domain,
            shape: self.into_product_shape(),
            context,
        }
    }
}

/// Project one graded formula into exactly `N` categorical cells.
///
/// Every interior formula knot must be an explicit partition cut. This makes
/// the formula affine (or constant) throughout each cell, so exact midpoint
/// evaluation is unambiguous and independently reproducible. Missing knots,
/// hostile partitions, arithmetic overflow, or an unrepresentable rational
/// payout are refused without producing a partial result.
pub fn project_to_categorical_v1<const N: usize>(
    domain: ScaledDomain,
    cut_points: Vec<i128>,
    shape: GradedShapeV1,
    failure_payout: ExactAmount,
    boundary: GradedRoundingBoundaryV1,
) -> Result<ProjectedCategoricalShapeV1<N>, CompileError> {
    let partition = CanonicalPartition::new(domain, cut_points)?;
    let price_region_count = partition.cell_count()?;
    let price_regions = usize::try_from(price_region_count)
        .map_err(|_| CompileError::CountOverflow)?;
    super::require_outcome_width::<N>(price_region_count)?;
    validate_shape(&partition, shape)?;
    validate_amount(failure_payout)?;

    let mut payouts = [ExactAmount {
        numerator: 0,
        denominator: 1,
    }; N];
    for (index, payout) in payouts.iter_mut().take(price_regions).enumerate() {
        let (left, right) = cell_bounds(&partition, index)?;
        *payout = match boundary {
            GradedRoundingBoundaryV1::CellMidpoint => midpoint_payout(shape, left, right)?,
        };
    }
    *payouts
        .get_mut(price_regions)
        .ok_or(CompileError::OutcomeCountMismatch)? = reduce(failure_payout)?;
    if payouts.iter().all(|payout| payout.numerator == 0) {
        return Err(CompileError::Contract(
            dclutch_product_contract::Error::EmptyPortfolioTemplate,
        ));
    }
    Ok(ProjectedCategoricalShapeV1 {
        partition,
        payouts,
        failure_payout,
        boundary,
    })
}

fn validate_shape(
    partition: &CanonicalPartition,
    shape: GradedShapeV1,
) -> Result<(), CompileError> {
    let domain = partition.domain();
    match shape {
        GradedShapeV1::CappedRamp { start, end, cap } => {
            validate_amount(cap)?;
            validate_ordered_knots(domain, &[start, end])?;
            require_interior_cuts(partition, &[start, end])
        }
        GradedShapeV1::Tent {
            start,
            peak,
            end,
            cap,
        } => {
            validate_amount(cap)?;
            validate_ordered_knots(domain, &[start, peak, end])?;
            require_interior_cuts(partition, &[start, peak, end])
        }
        GradedShapeV1::RangeBand { start, end, payout } => {
            validate_amount(payout)?;
            validate_ordered_knots(domain, &[start, end])?;
            if end == domain.upper {
                return Err(CompileError::InvalidKnot);
            }
            require_interior_cuts(partition, &[start, end])
        }
    }
}

fn validate_amount(amount: ExactAmount) -> Result<(), CompileError> {
    if amount.denominator == 0 {
        return Err(CompileError::ZeroPayoutDenominator);
    }
    Ok(())
}

fn validate_ordered_knots(domain: ScaledDomain, knots: &[i128]) -> Result<(), CompileError> {
    if domain.denominator == 0 {
        return Err(CompileError::ZeroCoordinateDenominator);
    }
    if domain.lower >= domain.upper {
        return Err(CompileError::InvalidDomain);
    }
    let mut previous = None;
    for knot in knots {
        if *knot <= domain.lower
            || *knot >= domain.upper
            || previous.is_some_and(|prior| *knot <= prior)
        {
            return Err(CompileError::InvalidKnot);
        }
        previous = Some(*knot);
    }
    Ok(())
}

fn require_interior_cuts(
    partition: &CanonicalPartition,
    knots: &[i128],
) -> Result<(), CompileError> {
    for knot in knots {
        if partition.cuts().binary_search(knot).is_err() {
            return Err(CompileError::ProjectionKnotMissing);
        }
    }
    Ok(())
}

fn cell_bounds(partition: &CanonicalPartition, index: usize) -> Result<(i128, i128), CompileError> {
    let left = if index == 0 {
        partition.domain().lower
    } else {
        partition
            .cuts()
            .get(index.saturating_sub(1))
            .copied()
            .ok_or(CompileError::OutcomeCountMismatch)?
    };
    let right = partition
        .cuts()
        .get(index)
        .copied()
        .unwrap_or(partition.domain().upper);
    if left >= right {
        return Err(CompileError::NonCanonicalPartition);
    }
    Ok((left, right))
}

fn midpoint_payout(
    shape: GradedShapeV1,
    left: i128,
    right: i128,
) -> Result<ExactAmount, CompileError> {
    match shape {
        GradedShapeV1::CappedRamp { start, end, cap } => {
            if right <= start {
                zero()
            } else if left >= end {
                reduce(cap)
            } else {
                let numerator = ascending_midpoint_numerator(left, right, start)?;
                let denominator = doubled_span(start, end)?;
                scale_amount(cap, numerator, denominator)
            }
        }
        GradedShapeV1::Tent {
            start,
            peak,
            end,
            cap,
        } => {
            if right <= start || left >= end {
                zero()
            } else if right <= peak {
                let numerator = ascending_midpoint_numerator(left, right, start)?;
                let denominator = doubled_span(start, peak)?;
                scale_amount(cap, numerator, denominator)
            } else if left >= peak {
                let numerator = descending_midpoint_numerator(left, right, end)?;
                let denominator = doubled_span(peak, end)?;
                scale_amount(cap, numerator, denominator)
            } else {
                Err(CompileError::ProjectionKnotMissing)
            }
        }
        GradedShapeV1::RangeBand { start, end, payout } => {
            if left >= start && right <= end {
                reduce(payout)
            } else if right <= start || left >= end {
                zero()
            } else {
                Err(CompileError::ProjectionKnotMissing)
            }
        }
    }
}

fn ascending_midpoint_numerator(
    left: i128,
    right: i128,
    start: i128,
) -> Result<u128, CompileError> {
    let left_delta = positive_or_zero_difference(left, start)?;
    let width = positive_difference(right, left)?;
    left_delta
        .checked_mul(2)
        .and_then(|value| value.checked_add(width))
        .ok_or(CompileError::ArithmeticOverflow)
}

fn descending_midpoint_numerator(left: i128, right: i128, end: i128) -> Result<u128, CompileError> {
    let left_to_end = positive_difference(end, left)?;
    let width = positive_difference(right, left)?;
    left_to_end
        .checked_mul(2)
        .and_then(|value| value.checked_sub(width))
        .ok_or(CompileError::ArithmeticOverflow)
}

fn doubled_span(left: i128, right: i128) -> Result<u128, CompileError> {
    positive_difference(right, left)?
        .checked_mul(2)
        .ok_or(CompileError::ArithmeticOverflow)
}

fn positive_difference(high: i128, low: i128) -> Result<u128, CompileError> {
    let difference = high
        .checked_sub(low)
        .ok_or(CompileError::ArithmeticOverflow)?;
    if difference <= 0 {
        return Err(CompileError::InvalidKnot);
    }
    u128::try_from(difference).map_err(|_| CompileError::ArithmeticOverflow)
}

fn positive_or_zero_difference(high: i128, low: i128) -> Result<u128, CompileError> {
    let difference = high
        .checked_sub(low)
        .ok_or(CompileError::ArithmeticOverflow)?;
    if difference < 0 {
        return Err(CompileError::ProjectionKnotMissing);
    }
    u128::try_from(difference).map_err(|_| CompileError::ArithmeticOverflow)
}

fn scale_amount(
    amount: ExactAmount,
    factor_numerator: u128,
    factor_denominator: u128,
) -> Result<ExactAmount, CompileError> {
    if factor_denominator == 0 {
        return Err(CompileError::ZeroPayoutDenominator);
    }
    let mut amount_numerator = u128::from(amount.numerator);
    let mut amount_denominator = u128::from(amount.denominator);
    let mut factor_numerator = factor_numerator;
    let mut factor_denominator = factor_denominator;
    let amount_divisor = gcd_u128(amount_numerator, amount_denominator);
    amount_numerator = amount_numerator
        .checked_div(amount_divisor)
        .ok_or(CompileError::ArithmeticOverflow)?;
    amount_denominator = amount_denominator
        .checked_div(amount_divisor)
        .ok_or(CompileError::ArithmeticOverflow)?;
    let factor_divisor = gcd_u128(factor_numerator, factor_denominator);
    factor_numerator = factor_numerator
        .checked_div(factor_divisor)
        .ok_or(CompileError::ArithmeticOverflow)?;
    factor_denominator = factor_denominator
        .checked_div(factor_divisor)
        .ok_or(CompileError::ArithmeticOverflow)?;
    let first = gcd_u128(amount_numerator, factor_denominator);
    amount_numerator = amount_numerator
        .checked_div(first)
        .ok_or(CompileError::ArithmeticOverflow)?;
    factor_denominator = factor_denominator
        .checked_div(first)
        .ok_or(CompileError::ArithmeticOverflow)?;
    let second = gcd_u128(factor_numerator, amount_denominator);
    factor_numerator = factor_numerator
        .checked_div(second)
        .ok_or(CompileError::ArithmeticOverflow)?;
    amount_denominator = amount_denominator
        .checked_div(second)
        .ok_or(CompileError::ArithmeticOverflow)?;
    let numerator = amount_numerator
        .checked_mul(factor_numerator)
        .ok_or(CompileError::ArithmeticOverflow)?;
    let denominator = amount_denominator
        .checked_mul(factor_denominator)
        .ok_or(CompileError::ArithmeticOverflow)?;
    exact_amount(numerator, denominator)
}

fn reduce(amount: ExactAmount) -> Result<ExactAmount, CompileError> {
    exact_amount(u128::from(amount.numerator), u128::from(amount.denominator))
}

fn zero() -> Result<ExactAmount, CompileError> {
    Ok(ExactAmount {
        numerator: 0,
        denominator: 1,
    })
}

fn exact_amount(numerator: u128, denominator: u128) -> Result<ExactAmount, CompileError> {
    if denominator == 0 {
        return Err(CompileError::ZeroPayoutDenominator);
    }
    if numerator == 0 {
        return zero();
    }
    let divisor = gcd_u128(numerator, denominator);
    let reduced_numerator = numerator
        .checked_div(divisor)
        .ok_or(CompileError::ArithmeticOverflow)?;
    let reduced_denominator = denominator
        .checked_div(divisor)
        .ok_or(CompileError::ArithmeticOverflow)?;
    Ok(ExactAmount {
        numerator: u64::try_from(reduced_numerator)
            .map_err(|_| CompileError::UnrepresentableProjectedPayout)?,
        denominator: u64::try_from(reduced_denominator)
            .map_err(|_| CompileError::UnrepresentableProjectedPayout)?,
    })
}

fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use dclutch_product_contract::ContentId;
    use dclutch_product_contract::capacity::{
        CapacityEnvelope, CapacityProfileId, CapacityProfileV1, CapacityProfileV1Input,
    };

    use super::*;
    use crate::{compile, recheck};

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero identity")
    }

    fn context() -> CompilationContext {
        let capacity_profile = CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Provisional,
            verifier_release_id: id(1),
            envelope_basis_id: id(2),
            max_artifact_bytes: 512,
            page_payload_bytes: 128,
            max_pages: 4,
            max_partition_cells: 16,
        })
        .expect("capacity profile");
        CompilationContext {
            capacity_profile,
            capacity_profile_id: CapacityProfileId::new(id(3)),
            terms_semantic_release_id: id(4),
            coordinate_domain_id: id(5),
            result_unit_id: id(6),
            occurrence_artifact: vec![5; 32],
        }
    }

    #[test]
    fn ramp_tent_and_range_project_to_exact_nonnegative_buckets() {
        let domain = ScaledDomain {
            lower: 0,
            upper: 40,
            denominator: 1,
        };
        let cuts = vec![10, 20, 30];
        let ramp = project_to_categorical_v1::<5>(
            domain,
            cuts.clone(),
            GradedShapeV1::CappedRamp {
                start: 10,
                end: 30,
                cap: ExactAmount {
                    numerator: 1,
                    denominator: 1,
                },
            },
            ExactAmount::ZERO,
            GradedRoundingBoundaryV1::CellMidpoint,
        )
        .expect("ramp projection");
        assert_eq!(
            ramp.payouts(),
            &[
                ExactAmount {
                    numerator: 0,
                    denominator: 1,
                },
                ExactAmount {
                    numerator: 1,
                    denominator: 4,
                },
                ExactAmount {
                    numerator: 3,
                    denominator: 4,
                },
                ExactAmount {
                    numerator: 1,
                    denominator: 1,
                },
                ExactAmount::ZERO,
            ]
        );

        let tent = project_to_categorical_v1::<5>(
            domain,
            cuts.clone(),
            GradedShapeV1::Tent {
                start: 10,
                peak: 20,
                end: 30,
                cap: ExactAmount {
                    numerator: 2,
                    denominator: 1,
                },
            },
            ExactAmount::ZERO,
            GradedRoundingBoundaryV1::CellMidpoint,
        )
        .expect("tent projection");
        assert_eq!(
            tent.payouts(),
            &[
                ExactAmount {
                    numerator: 0,
                    denominator: 1,
                },
                ExactAmount {
                    numerator: 1,
                    denominator: 1,
                },
                ExactAmount {
                    numerator: 1,
                    denominator: 1,
                },
                ExactAmount {
                    numerator: 0,
                    denominator: 1,
                },
                ExactAmount::ZERO,
            ]
        );

        let range = project_to_categorical_v1::<5>(
            domain,
            cuts,
            GradedShapeV1::RangeBand {
                start: 10,
                end: 30,
                payout: ExactAmount {
                    numerator: 3,
                    denominator: 2,
                },
            },
            ExactAmount::ZERO,
            GradedRoundingBoundaryV1::CellMidpoint,
        )
        .expect("range projection");
        assert_eq!(
            range.payouts(),
            &[
                ExactAmount {
                    numerator: 0,
                    denominator: 1,
                },
                ExactAmount {
                    numerator: 3,
                    denominator: 2,
                },
                ExactAmount {
                    numerator: 3,
                    denominator: 2,
                },
                ExactAmount {
                    numerator: 0,
                    denominator: 1,
                },
                ExactAmount::ZERO,
            ]
        );
    }

    #[test]
    fn projected_shape_compiles_and_rechecks_through_existing_liabilities() {
        let domain = ScaledDomain {
            lower: -20,
            upper: 20,
            denominator: 10,
        };
        let projection = project_to_categorical_v1::<5>(
            domain,
            vec![-10, 0, 10],
            GradedShapeV1::Tent {
                start: -10,
                peak: 0,
                end: 10,
                cap: ExactAmount {
                    numerator: 1,
                    denominator: 1,
                },
            },
            ExactAmount::ZERO,
            GradedRoundingBoundaryV1::CellMidpoint,
        )
        .expect("projection");
        let request = projection.into_compile_request(context());
        let compiled = compile(&request).expect("categorical compile");
        assert_eq!(compiled.claim_basis.outcome_count(), 5);
        assert_eq!(compiled.portfolio_template.coefficients(), &[0, 1, 1, 0, 0]);
        assert_eq!(compiled.portfolio_template.denominator(), 2);
        assert_eq!(recheck(&request, &compiled), Ok(()));
    }

    #[test]
    fn missing_knots_bad_width_and_unrepresentable_arithmetic_refuse() {
        let domain = ScaledDomain {
            lower: 0,
            upper: 40,
            denominator: 1,
        };
        assert_eq!(
            project_to_categorical_v1::<4>(
                domain,
                vec![10, 30],
                GradedShapeV1::Tent {
                    start: 10,
                    peak: 20,
                    end: 30,
                    cap: ExactAmount {
                        numerator: 1,
                        denominator: 1,
                    },
                },
                ExactAmount::ZERO,
                GradedRoundingBoundaryV1::CellMidpoint,
            ),
            Err(CompileError::ProjectionKnotMissing)
        );
        assert_eq!(
            project_to_categorical_v1::<4>(
                domain,
                vec![10, 20, 30],
                GradedShapeV1::CappedRamp {
                    start: 10,
                    end: 30,
                    cap: ExactAmount {
                        numerator: 1,
                        denominator: 1,
                    },
                },
                ExactAmount::ZERO,
                GradedRoundingBoundaryV1::CellMidpoint,
            ),
            Err(CompileError::OutcomeCountMismatch)
        );
        assert_eq!(
            project_to_categorical_v1::<4>(
                ScaledDomain {
                    lower: i128::MIN,
                    upper: i128::MAX,
                    denominator: 1,
                },
                vec![i128::MIN + 1, i128::MAX - 1],
                GradedShapeV1::CappedRamp {
                    start: i128::MIN + 1,
                    end: i128::MAX - 1,
                    cap: ExactAmount {
                        numerator: u64::MAX,
                        denominator: 1,
                    },
                },
                ExactAmount::ZERO,
                GradedRoundingBoundaryV1::CellMidpoint,
            ),
            Err(CompileError::ArithmeticOverflow)
        );
        assert_eq!(
            project_to_categorical_v1::<5>(
                ScaledDomain {
                    lower: -1,
                    upper: i128::from(u64::MAX) + 1,
                    denominator: 1,
                },
                vec![0, 1, i128::from(u64::MAX)],
                GradedShapeV1::CappedRamp {
                    start: 0,
                    end: i128::from(u64::MAX),
                    cap: ExactAmount {
                        numerator: 1,
                        denominator: 1,
                    },
                },
                ExactAmount::ZERO,
                GradedRoundingBoundaryV1::CellMidpoint,
            ),
            Err(CompileError::UnrepresentableProjectedPayout)
        );
    }

    #[test]
    fn cross_cancellation_admits_representable_large_rational() {
        let magnitude = 1i128 << 126;
        let projection = project_to_categorical_v1::<4>(
            ScaledDomain {
                lower: -1,
                upper: magnitude + 2,
                denominator: 1,
            },
            vec![0, magnitude],
            GradedShapeV1::CappedRamp {
                start: 0,
                end: magnitude,
                cap: ExactAmount {
                    numerator: u64::MAX,
                    denominator: u64::MAX,
                },
            },
            ExactAmount::ZERO,
            GradedRoundingBoundaryV1::CellMidpoint,
        )
        .expect("representable after cancellation");
        assert_eq!(
            projection.payouts(),
            &[
                ExactAmount::ZERO,
                ExactAmount {
                    numerator: 1,
                    denominator: 2,
                },
                ExactAmount {
                    numerator: 1,
                    denominator: 1,
                },
                ExactAmount::ZERO,
            ]
        );
    }

    #[test]
    fn empty_projection_and_exclusive_upper_band_refuse() {
        let domain = ScaledDomain {
            lower: 0,
            upper: 10,
            denominator: 1,
        };
        assert_eq!(
            project_to_categorical_v1::<4>(
                domain,
                vec![2, 5],
                GradedShapeV1::CappedRamp {
                    start: 2,
                    end: 5,
                    cap: ExactAmount::ZERO,
                },
                ExactAmount::ZERO,
                GradedRoundingBoundaryV1::CellMidpoint,
            ),
            Err(CompileError::Contract(
                dclutch_product_contract::Error::EmptyPortfolioTemplate
            ))
        );
        assert_eq!(
            project_to_categorical_v1::<3>(
                domain,
                vec![5],
                GradedShapeV1::RangeBand {
                    start: 5,
                    end: 10,
                    payout: ExactAmount {
                        numerator: 1,
                        denominator: 1,
                    },
                },
                ExactAmount::ZERO,
                GradedRoundingBoundaryV1::CellMidpoint,
            ),
            Err(CompileError::InvalidKnot)
        );
        assert_eq!(
            project_to_categorical_v1::<3>(
                domain,
                vec![5],
                GradedShapeV1::CappedRamp {
                    start: 0,
                    end: 5,
                    cap: ExactAmount {
                        numerator: 1,
                        denominator: 1,
                    },
                },
                ExactAmount::ZERO,
                GradedRoundingBoundaryV1::CellMidpoint,
            ),
            Err(CompileError::InvalidKnot)
        );
        assert_eq!(
            project_to_categorical_v1::<4>(
                domain,
                vec![2, 5],
                GradedShapeV1::Tent {
                    start: 2,
                    peak: 5,
                    end: 10,
                    cap: ExactAmount {
                        numerator: 1,
                        denominator: 1,
                    },
                },
                ExactAmount::ZERO,
                GradedRoundingBoundaryV1::CellMidpoint,
            ),
            Err(CompileError::InvalidKnot)
        );
    }
}
