//! Exact representative-point payoff compilation and comparison.
//!
//! This module accepts a bounded scalar partition that is exhaustive,
//! disjoint, ordered, and canonical over its registered coordinate domain. It
//! compiles useful shapes at each exact representative point into native Egg
//! coefficients. Representative-point semantics are deliberate: the module
//! does not claim that a shape is constant everywhere inside a cell. For a
//! smooth native basis these values are control coefficients, not a claim that
//! the final spline interpolates every representative.

use crate::{
    certify_portfolio_compression_v1, is_zero_identity, CompressionUnitModelV1, Error,
    NativeEggPortfolioV1, PortfolioCompressionV1, PortfolioDomainV1, Result, MAX_OUTCOMES,
};

/// Maximum boundary slots for a sixteen-cell exhaustive partition.
pub const MAX_PARTITION_BOUNDARIES: usize = MAX_OUTCOMES + 1;

/// One exact bounded scalar partition and its representative points.
///
/// Active cell `i` is `[boundary[i], boundary[i + 1])`, except the final cell,
/// which includes the registered upper endpoint. Strictly increasing shared
/// boundaries make the cells disjoint and exhaustive over the bounded domain.
/// Every coordinate uses the exact integer statistic unit frozen by Terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExhaustivePartitionV1 {
    portfolio_domain: PortfolioDomainV1,
    partition_id: [u8; 32],
    domain_lower_point: i64,
    domain_upper_point: i64,
    boundaries: [i64; MAX_PARTITION_BOUNDARIES],
    representative_points: [i64; MAX_OUTCOMES],
}

impl ExhaustivePartitionV1 {
    /// Validate the complete bounded partition and its exact representatives.
    pub fn new(
        portfolio_domain: PortfolioDomainV1,
        partition_id: [u8; 32],
        domain_lower_point: i64,
        domain_upper_point: i64,
        boundaries: [i64; MAX_PARTITION_BOUNDARIES],
        representative_points: [i64; MAX_OUTCOMES],
    ) -> Result<Self> {
        if is_zero_identity(&partition_id) {
            return Err(Error::ZeroIdentity);
        }
        let active = usize::from(portfolio_domain.outcome_count());
        if domain_lower_point >= domain_upper_point
            || boundaries[0] != domain_lower_point
            || boundaries[active] != domain_upper_point
        {
            return Err(Error::NoncanonicalPartition);
        }

        let mut cell = 0usize;
        while cell < active {
            let lower = boundaries[cell];
            let upper = boundaries[cell + 1];
            if lower >= upper {
                return Err(Error::NoncanonicalPartition);
            }
            let representative = representative_points[cell];
            let inside = if cell + 1 == active {
                representative >= lower && representative <= upper
            } else {
                representative >= lower && representative < upper
            };
            if !inside {
                return Err(Error::InvalidRepresentativePoint);
            }
            cell += 1;
        }

        let mut boundary = active + 1;
        while boundary < MAX_PARTITION_BOUNDARIES {
            if boundaries[boundary] != 0 {
                return Err(Error::NoncanonicalPartition);
            }
            boundary += 1;
        }
        let mut representative = active;
        while representative < MAX_OUTCOMES {
            if representative_points[representative] != 0 {
                return Err(Error::NoncanonicalPartition);
            }
            representative += 1;
        }

        Ok(Self {
            portfolio_domain,
            partition_id,
            domain_lower_point,
            domain_upper_point,
            boundaries,
            representative_points,
        })
    }

    /// Market/Terms/width capability whose native Eggs this partition names.
    pub const fn portfolio_domain(&self) -> PortfolioDomainV1 {
        self.portfolio_domain
    }

    /// Adapter-authenticated identity expected to commit this exact partition.
    pub const fn partition_id(&self) -> [u8; 32] {
        self.partition_id
    }

    /// Inclusive lower endpoint in the Terms-frozen integer statistic unit.
    pub const fn domain_lower_point(&self) -> i64 {
        self.domain_lower_point
    }

    /// Inclusive upper endpoint in the Terms-frozen integer statistic unit.
    pub const fn domain_upper_point(&self) -> i64 {
        self.domain_upper_point
    }

    /// Active shared boundaries followed by canonical zero padding.
    pub const fn boundaries(&self) -> &[i64; MAX_PARTITION_BOUNDARIES] {
        &self.boundaries
    }

    /// One exact active representative per cell, then canonical zero padding.
    pub const fn representative_points(&self) -> &[i64; MAX_OUTCOMES] {
        &self.representative_points
    }
}

/// Direction of a digital tail payoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TailDirectionV1 {
    /// Pay representatives less than or equal to the threshold.
    LowerInclusive = 0,
    /// Pay representatives greater than or equal to the threshold.
    UpperInclusive = 1,
}

/// Direction of a capped monotone linear payoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CappedLinearDirectionV1 {
    /// Floor at and below `start`, then increase to the cap at `end`.
    Increasing = 0,
    /// Cap at and below `start`, then decrease to the floor at `end`.
    Decreasing = 1,
}

/// Bounded shape compiled exactly at partition representative points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayoffShapeV1 {
    /// Fixed nonzero payout on one inclusive tail.
    DigitalTail {
        /// Selected lower or upper tail.
        direction: TailDirectionV1,
        /// Exact threshold in the Terms-frozen integer statistic unit.
        threshold_point: i64,
        /// Native Egg atoms paid on selected representatives.
        payout_atoms: u64,
    },
    /// Fixed nonzero payout inside an inclusive representative-point range.
    Range {
        /// Inclusive exact lower point in the frozen statistic unit.
        lower_point: i64,
        /// Inclusive exact upper point in the frozen statistic unit.
        upper_point: i64,
        /// Native Egg atoms paid inside the range.
        payout_atoms: u64,
    },
    /// Bounded monotone linear ramp with exact integer coefficients.
    CappedMonotoneLinear {
        /// Increasing or decreasing ramp orientation.
        direction: CappedLinearDirectionV1,
        /// Exact first ramp coordinate in the frozen statistic unit.
        start_point: i64,
        /// Exact last ramp coordinate; strictly above `start_point`.
        end_point: i64,
        /// Nonnegative payout at the low side of an increasing ramp and high
        /// side of a decreasing ramp.
        floor_atoms: u64,
        /// Strictly larger payout at the high side of an increasing ramp and
        /// low side of a decreasing ramp.
        cap_atoms: u64,
    },
}

/// One exact shape compiled into a domain- and partition-bound Egg portfolio.
///
/// One later `position_unit` means one whole instance of the shape with its
/// declared payout atoms. Coefficients are therefore preserved exactly rather
/// than divided by a GCD and silently changing the position-unit definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledPayoffV1 {
    partition: ExhaustivePartitionV1,
    shape: PayoffShapeV1,
    portfolio: NativeEggPortfolioV1,
    minimum_coefficient_atoms: u64,
    maximum_coefficient_atoms: u64,
}

impl CompiledPayoffV1 {
    /// Complete exact partition capability used during compilation.
    pub const fn partition(&self) -> &ExhaustivePartitionV1 {
        &self.partition
    }

    /// Frozen source shape whose representative-point values were compiled.
    pub const fn shape(&self) -> PayoffShapeV1 {
        self.shape
    }

    /// Canonical native Egg portfolio produced by compilation.
    pub const fn portfolio(&self) -> NativeEggPortfolioV1 {
        self.portfolio
    }

    /// Minimum active Egg coefficient, the complete-set layer per unit.
    pub const fn minimum_coefficient_atoms(&self) -> u64 {
        self.minimum_coefficient_atoms
    }

    /// Maximum active Egg coefficient.
    pub const fn maximum_coefficient_atoms(&self) -> u64 {
        self.maximum_coefficient_atoms
    }

    /// Exact per-unit contingent coefficient range.
    pub const fn contingent_range_atoms(&self) -> u64 {
        self.maximum_coefficient_atoms - self.minimum_coefficient_atoms
    }
}

/// Partition-bound compression of one compiled payoff position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledPayoffCompressionV1 {
    partition: ExhaustivePartitionV1,
    compression: PortfolioCompressionV1,
}

impl CompiledPayoffCompressionV1 {
    /// Complete partition capability retained through compression.
    pub const fn partition(&self) -> &ExhaustivePartitionV1 {
        &self.partition
    }

    /// Checked maximal complete-set decomposition.
    pub const fn compression(&self) -> &PortfolioCompressionV1 {
        &self.compression
    }
}

/// Exact pointwise relation between two aggregate compiled payoff positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PayoffRelationV1 {
    /// Every active Egg-atom amount is equal.
    Equal = 0,
    /// The left amount is no smaller everywhere and larger somewhere.
    LeftDominates = 1,
    /// The right amount is no smaller everywhere and larger somewhere.
    RightDominates = 2,
    /// Each side is larger on at least one active Egg coordinate.
    Incomparable = 3,
}

/// Checked comparison and minimal complete-set make-whole quantities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactPayoffComparisonV1 {
    partition: ExhaustivePartitionV1,
    unit_model: CompressionUnitModelV1,
    left_position_units: u64,
    right_position_units: u64,
    relation: PayoffRelationV1,
    left_position_egg_atoms: [u64; MAX_OUTCOMES],
    right_position_egg_atoms: [u64; MAX_OUTCOMES],
    left_make_whole_atoms: u64,
    right_make_whole_atoms: u64,
}

impl ExactPayoffComparisonV1 {
    /// Complete partition capability shared by both compared payoffs.
    pub const fn partition(&self) -> &ExhaustivePartitionV1 {
        &self.partition
    }

    /// Frozen one-Egg-per-set and one-collateral-per-Split atom model.
    pub const fn unit_model(&self) -> CompressionUnitModelV1 {
        self.unit_model
    }

    /// Aggregate unit count of the left compiled payoff.
    pub const fn left_position_units(&self) -> u64 {
        self.left_position_units
    }

    /// Aggregate unit count of the right compiled payoff.
    pub const fn right_position_units(&self) -> u64 {
        self.right_position_units
    }

    /// Exact pointwise relation over the active native Egg coordinates.
    pub const fn relation(&self) -> PayoffRelationV1 {
        self.relation
    }

    /// Checked aggregate left Egg-atom vector.
    pub const fn left_position_egg_atoms(&self) -> &[u64; MAX_OUTCOMES] {
        &self.left_position_egg_atoms
    }

    /// Checked aggregate right Egg-atom vector.
    pub const fn right_position_egg_atoms(&self) -> &[u64; MAX_OUTCOMES] {
        &self.right_position_egg_atoms
    }

    /// Minimum complete sets, and thus collateral atoms, which must be added
    /// to the left position to make it pointwise dominate the right.
    pub const fn left_make_whole_atoms(&self) -> u64 {
        self.left_make_whole_atoms
    }

    /// Minimum complete sets, and thus collateral atoms, which must be added
    /// to the right position to make it pointwise dominate the left.
    pub const fn right_make_whole_atoms(&self) -> u64 {
        self.right_make_whole_atoms
    }
}

/// Compile one bounded shape exactly at every active representative point.
///
/// Digital and range shapes use inclusive comparisons. A linear interior is
/// admitted only when its exact rational value is already a whole Egg atom;
/// this compiler has no rounding boundary.
pub fn compile_payoff_shape_v1(
    partition: ExhaustivePartitionV1,
    shape: PayoffShapeV1,
) -> Result<CompiledPayoffV1> {
    validate_shape(shape)?;
    let active = usize::from(partition.portfolio_domain.outcome_count());
    let mut coefficients = [0u64; MAX_OUTCOMES];
    let mut minimum = u64::MAX;
    let mut maximum = 0u64;
    let mut outcome = 0usize;
    while outcome < active {
        let coefficient = evaluate_shape(shape, partition.representative_points[outcome])?;
        coefficients[outcome] = coefficient;
        if coefficient < minimum {
            minimum = coefficient;
        }
        if coefficient > maximum {
            maximum = coefficient;
        }
        outcome += 1;
    }
    let portfolio = NativeEggPortfolioV1::new(partition.portfolio_domain, coefficients)?;
    Ok(CompiledPayoffV1 {
        partition,
        shape,
        portfolio,
        minimum_coefficient_atoms: minimum,
        maximum_coefficient_atoms: maximum,
    })
}

/// Compress a compiled position while retaining its exact partition binding.
pub fn certify_compiled_payoff_compression_v1(
    compiled: CompiledPayoffV1,
    position_units: u64,
) -> Result<CompiledPayoffCompressionV1> {
    let compression = certify_portfolio_compression_v1(compiled.portfolio, position_units)?;
    Ok(CompiledPayoffCompressionV1 {
        partition: compiled.partition,
        compression,
    })
}

/// Compare two compiled aggregate payoffs and derive exact make-whole layers.
///
/// Adding `left_make_whole_atoms` complete sets to the left is the minimum
/// whole-atom constant layer that makes the left position pointwise dominate
/// the right. The symmetric field has the corresponding right-to-left
/// meaning. Checked reconstruction refuses a make-whole layer that could not
/// fit the Position atom width. No Split or transfer is authorized here.
pub fn compare_compiled_payoffs_v1(
    left: CompiledPayoffV1,
    left_position_units: u64,
    right: CompiledPayoffV1,
    right_position_units: u64,
) -> Result<ExactPayoffComparisonV1> {
    if left.partition != right.partition {
        return Err(Error::MismatchedPartition);
    }
    if left_position_units == 0 || right_position_units == 0 {
        return Err(Error::ZeroPositionUnits);
    }

    let active = usize::from(left.partition.portfolio_domain.outcome_count());
    let mut left_atoms = [0u64; MAX_OUTCOMES];
    let mut right_atoms = [0u64; MAX_OUTCOMES];
    let mut left_is_at_least = true;
    let mut right_is_at_least = true;
    let mut left_make_whole_atoms = 0u64;
    let mut right_make_whole_atoms = 0u64;
    let mut outcome = 0usize;
    while outcome < active {
        let left_amount = left.portfolio.egg_coefficients()[outcome]
            .checked_mul(left_position_units)
            .ok_or(Error::ArithmeticOverflow)?;
        let right_amount = right.portfolio.egg_coefficients()[outcome]
            .checked_mul(right_position_units)
            .ok_or(Error::ArithmeticOverflow)?;
        left_atoms[outcome] = left_amount;
        right_atoms[outcome] = right_amount;
        left_is_at_least &= left_amount >= right_amount;
        right_is_at_least &= right_amount >= left_amount;
        if right_amount > left_amount {
            let shortfall = right_amount - left_amount;
            if shortfall > left_make_whole_atoms {
                left_make_whole_atoms = shortfall;
            }
        }
        if left_amount > right_amount {
            let shortfall = left_amount - right_amount;
            if shortfall > right_make_whole_atoms {
                right_make_whole_atoms = shortfall;
            }
        }
        outcome += 1;
    }

    outcome = 0;
    while outcome < active {
        left_atoms[outcome]
            .checked_add(left_make_whole_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        right_atoms[outcome]
            .checked_add(right_make_whole_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        outcome += 1;
    }

    let relation = match (left_is_at_least, right_is_at_least) {
        (true, true) => PayoffRelationV1::Equal,
        (true, false) => PayoffRelationV1::LeftDominates,
        (false, true) => PayoffRelationV1::RightDominates,
        (false, false) => PayoffRelationV1::Incomparable,
    };
    Ok(ExactPayoffComparisonV1 {
        partition: left.partition,
        unit_model: CompressionUnitModelV1::NativeEggAtomParity,
        left_position_units,
        right_position_units,
        relation,
        left_position_egg_atoms: left_atoms,
        right_position_egg_atoms: right_atoms,
        left_make_whole_atoms,
        right_make_whole_atoms,
    })
}

fn validate_shape(shape: PayoffShapeV1) -> Result<()> {
    match shape {
        PayoffShapeV1::DigitalTail { payout_atoms, .. }
        | PayoffShapeV1::Range { payout_atoms, .. } => {
            if payout_atoms == 0 {
                return Err(Error::InvalidPayoffShape);
            }
        }
        PayoffShapeV1::CappedMonotoneLinear {
            start_point,
            end_point,
            floor_atoms,
            cap_atoms,
            ..
        } => {
            if start_point >= end_point || floor_atoms >= cap_atoms {
                return Err(Error::InvalidPayoffShape);
            }
        }
    }
    if let PayoffShapeV1::Range {
        lower_point,
        upper_point,
        ..
    } = shape
    {
        if lower_point > upper_point {
            return Err(Error::InvalidPayoffShape);
        }
    }
    Ok(())
}

fn evaluate_shape(shape: PayoffShapeV1, point: i64) -> Result<u64> {
    match shape {
        PayoffShapeV1::DigitalTail {
            direction,
            threshold_point,
            payout_atoms,
        } => {
            let selected = match direction {
                TailDirectionV1::LowerInclusive => point <= threshold_point,
                TailDirectionV1::UpperInclusive => point >= threshold_point,
            };
            Ok(if selected { payout_atoms } else { 0 })
        }
        PayoffShapeV1::Range {
            lower_point,
            upper_point,
            payout_atoms,
        } => Ok(if point >= lower_point && point <= upper_point {
            payout_atoms
        } else {
            0
        }),
        PayoffShapeV1::CappedMonotoneLinear {
            direction,
            start_point,
            end_point,
            floor_atoms,
            cap_atoms,
        } => evaluate_capped_linear(
            direction,
            point,
            start_point,
            end_point,
            floor_atoms,
            cap_atoms,
        ),
    }
}

fn evaluate_capped_linear(
    direction: CappedLinearDirectionV1,
    point: i64,
    start_point: i64,
    end_point: i64,
    floor_atoms: u64,
    cap_atoms: u64,
) -> Result<u64> {
    if point <= start_point {
        return Ok(match direction {
            CappedLinearDirectionV1::Increasing => floor_atoms,
            CappedLinearDirectionV1::Decreasing => cap_atoms,
        });
    }
    if point >= end_point {
        return Ok(match direction {
            CappedLinearDirectionV1::Increasing => cap_atoms,
            CappedLinearDirectionV1::Decreasing => floor_atoms,
        });
    }

    let point_offset = u128::try_from(i128::from(point) - i128::from(start_point))
        .map_err(|_| Error::ArithmeticOverflow)?;
    let coordinate_width = u128::try_from(i128::from(end_point) - i128::from(start_point))
        .map_err(|_| Error::ArithmeticOverflow)?;
    let payout_width = u128::from(
        cap_atoms
            .checked_sub(floor_atoms)
            .ok_or(Error::InvariantViolation)?,
    );
    let numerator = payout_width
        .checked_mul(point_offset)
        .ok_or(Error::ArithmeticOverflow)?;
    if numerator % coordinate_width != 0 {
        return Err(Error::InexactPayoffCoefficient);
    }
    let step = u64::try_from(numerator / coordinate_width)
        .map_err(|_| Error::ArithmeticOverflow)?;
    match direction {
        CappedLinearDirectionV1::Increasing => floor_atoms
            .checked_add(step)
            .ok_or(Error::ArithmeticOverflow),
        CappedLinearDirectionV1::Decreasing => {
            cap_atoms.checked_sub(step).ok_or(Error::InvariantViolation)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(outcome_count: u8) -> PortfolioDomainV1 {
        PortfolioDomainV1::new([1u8; 32], [2u8; 32], outcome_count).unwrap()
    }

    fn partition_five() -> ExhaustivePartitionV1 {
        let mut boundaries = [0i64; MAX_PARTITION_BOUNDARIES];
        boundaries[..6].copy_from_slice(&[-10, -3, 0, 2, 11, 20]);
        let mut representatives = [0i64; MAX_OUTCOMES];
        representatives[..5].copy_from_slice(&[-10, -1, 1, 7, 20]);
        ExhaustivePartitionV1::new(
            domain(5),
            [3u8; 32],
            -10,
            20,
            boundaries,
            representatives,
        )
        .unwrap()
    }

    fn partition_three(partition_byte: u8) -> ExhaustivePartitionV1 {
        let mut boundaries = [0i64; MAX_PARTITION_BOUNDARIES];
        boundaries[..4].copy_from_slice(&[-1, 1, 2, 3]);
        let mut representatives = [0i64; MAX_OUTCOMES];
        representatives[..3].copy_from_slice(&[0, 1, 2]);
        ExhaustivePartitionV1::new(
            domain(3),
            [partition_byte; 32],
            -1,
            3,
            boundaries,
            representatives,
        )
        .unwrap()
    }

    #[test]
    fn arbitrary_width_tail_and_range_shapes_compile_exactly() {
        let partition = partition_five();
        let upper_tail = compile_payoff_shape_v1(
            partition,
            PayoffShapeV1::DigitalTail {
                direction: TailDirectionV1::UpperInclusive,
                threshold_point: 1,
                payout_atoms: 4,
            },
        )
        .unwrap();
        assert_eq!(
            &upper_tail.portfolio().egg_coefficients()[..5],
            &[0, 0, 4, 4, 4]
        );
        assert_eq!(upper_tail.minimum_coefficient_atoms(), 0);
        assert_eq!(upper_tail.maximum_coefficient_atoms(), 4);
        assert_eq!(upper_tail.contingent_range_atoms(), 4);

        let lower_tail = compile_payoff_shape_v1(
            partition,
            PayoffShapeV1::DigitalTail {
                direction: TailDirectionV1::LowerInclusive,
                threshold_point: 0,
                payout_atoms: 3,
            },
        )
        .unwrap();
        assert_eq!(
            &lower_tail.portfolio().egg_coefficients()[..5],
            &[3, 3, 0, 0, 0]
        );

        let range = compile_payoff_shape_v1(
            partition,
            PayoffShapeV1::Range {
                lower_point: -1,
                upper_point: 7,
                payout_atoms: 5,
            },
        )
        .unwrap();
        assert_eq!(
            &range.portfolio().egg_coefficients()[..5],
            &[0, 5, 5, 5, 0]
        );
    }

    #[test]
    fn capped_linear_shapes_compile_without_rounding_and_retain_compression() {
        let partition = partition_five();
        let increasing = compile_payoff_shape_v1(
            partition,
            PayoffShapeV1::CappedMonotoneLinear {
                direction: CappedLinearDirectionV1::Increasing,
                start_point: -1,
                end_point: 7,
                floor_atoms: 1,
                cap_atoms: 5,
            },
        )
        .unwrap();
        assert_eq!(
            &increasing.portfolio().egg_coefficients()[..5],
            &[1, 1, 2, 5, 5]
        );
        assert_eq!(increasing.minimum_coefficient_atoms(), 1);
        assert_eq!(increasing.maximum_coefficient_atoms(), 5);

        let compression = certify_compiled_payoff_compression_v1(increasing, 3).unwrap();
        assert_eq!(compression.partition(), &partition);
        assert_eq!(
            compression.compression().recoverable_collateral_atoms(),
            3
        );
        assert_eq!(
            &compression.compression().residual_position_egg_atoms()[..5],
            &[0, 0, 3, 12, 12]
        );

        let decreasing = compile_payoff_shape_v1(
            partition,
            PayoffShapeV1::CappedMonotoneLinear {
                direction: CappedLinearDirectionV1::Decreasing,
                start_point: -1,
                end_point: 7,
                floor_atoms: 1,
                cap_atoms: 5,
            },
        )
        .unwrap();
        assert_eq!(
            &decreasing.portfolio().egg_coefficients()[..5],
            &[5, 5, 4, 1, 1]
        );
    }

    #[test]
    fn fractional_linear_coefficients_are_refused_instead_of_rounded() {
        let partition = partition_three(4);
        assert_eq!(
            compile_payoff_shape_v1(
                partition,
                PayoffShapeV1::CappedMonotoneLinear {
                    direction: CappedLinearDirectionV1::Increasing,
                    start_point: 0,
                    end_point: 3,
                    floor_atoms: 0,
                    cap_atoms: 2,
                },
            ),
            Err(Error::InexactPayoffCoefficient)
        );
    }

    #[test]
    fn partition_constructor_refuses_noncanonical_or_nonmember_cells() {
        let mut boundaries = [0i64; MAX_PARTITION_BOUNDARIES];
        boundaries[..4].copy_from_slice(&[-1, 1, 2, 3]);
        let mut representatives = [0i64; MAX_OUTCOMES];
        representatives[..3].copy_from_slice(&[0, 1, 2]);

        assert_eq!(
            ExhaustivePartitionV1::new(
                domain(3),
                [0u8; 32],
                -1,
                3,
                boundaries,
                representatives,
            ),
            Err(Error::ZeroIdentity)
        );
        let mut unordered = boundaries;
        unordered[2] = 1;
        assert_eq!(
            ExhaustivePartitionV1::new(
                domain(3),
                [4u8; 32],
                -1,
                3,
                unordered,
                representatives,
            ),
            Err(Error::NoncanonicalPartition)
        );

        let mut outsider = representatives;
        outsider[0] = 1;
        assert_eq!(
            ExhaustivePartitionV1::new(
                domain(3),
                [4u8; 32],
                -1,
                3,
                boundaries,
                outsider,
            ),
            Err(Error::InvalidRepresentativePoint)
        );

        let mut padded = boundaries;
        padded[4] = 7;
        assert_eq!(
            ExhaustivePartitionV1::new(
                domain(3),
                [4u8; 32],
                -1,
                3,
                padded,
                representatives,
            ),
            Err(Error::NoncanonicalPartition)
        );
    }

    #[test]
    fn invalid_or_empty_shapes_refuse_before_becoming_portfolios() {
        let partition = partition_three(4);
        assert_eq!(
            compile_payoff_shape_v1(
                partition,
                PayoffShapeV1::Range {
                    lower_point: 2,
                    upper_point: 1,
                    payout_atoms: 1,
                },
            ),
            Err(Error::InvalidPayoffShape)
        );
        assert_eq!(
            compile_payoff_shape_v1(
                partition,
                PayoffShapeV1::DigitalTail {
                    direction: TailDirectionV1::UpperInclusive,
                    threshold_point: 0,
                    payout_atoms: 0,
                },
            ),
            Err(Error::InvalidPayoffShape)
        );
        assert_eq!(
            compile_payoff_shape_v1(
                partition,
                PayoffShapeV1::CappedMonotoneLinear {
                    direction: CappedLinearDirectionV1::Increasing,
                    start_point: 2,
                    end_point: 2,
                    floor_atoms: 0,
                    cap_atoms: 1,
                },
            ),
            Err(Error::InvalidPayoffShape)
        );
        assert_eq!(
            compile_payoff_shape_v1(
                partition,
                PayoffShapeV1::DigitalTail {
                    direction: TailDirectionV1::UpperInclusive,
                    threshold_point: 99,
                    payout_atoms: 1,
                },
            ),
            Err(Error::ZeroPayoff)
        );
    }

    #[test]
    fn comparison_classifies_relations_and_exact_bidirectional_make_wholes() {
        let partition = partition_three(4);
        let left = compile_payoff_shape_v1(
            partition,
            PayoffShapeV1::Range {
                lower_point: 0,
                upper_point: 1,
                payout_atoms: 2,
            },
        )
        .unwrap();
        let right = compile_payoff_shape_v1(
            partition,
            PayoffShapeV1::DigitalTail {
                direction: TailDirectionV1::UpperInclusive,
                threshold_point: 1,
                payout_atoms: 1,
            },
        )
        .unwrap();
        let incomparable = compare_compiled_payoffs_v1(left, 1, right, 1).unwrap();
        assert_eq!(
            incomparable.unit_model(),
            CompressionUnitModelV1::NativeEggAtomParity
        );
        assert_eq!(incomparable.relation(), PayoffRelationV1::Incomparable);
        assert_eq!(incomparable.left_make_whole_atoms(), 1);
        assert_eq!(incomparable.right_make_whole_atoms(), 2);
        assert_eq!(
            &incomparable.left_position_egg_atoms()[..3],
            &[2, 2, 0]
        );
        assert_eq!(
            &incomparable.right_position_egg_atoms()[..3],
            &[0, 1, 1]
        );

        let equal = compare_compiled_payoffs_v1(left, 3, left, 3).unwrap();
        assert_eq!(equal.relation(), PayoffRelationV1::Equal);
        assert_eq!(equal.left_make_whole_atoms(), 0);
        assert_eq!(equal.right_make_whole_atoms(), 0);

        let complete_set = compile_payoff_shape_v1(
            partition,
            PayoffShapeV1::DigitalTail {
                direction: TailDirectionV1::UpperInclusive,
                threshold_point: 0,
                payout_atoms: 3,
            },
        )
        .unwrap();
        let dominates = compare_compiled_payoffs_v1(complete_set, 1, left, 1).unwrap();
        assert_eq!(dominates.relation(), PayoffRelationV1::LeftDominates);
        assert_eq!(dominates.left_make_whole_atoms(), 0);
        assert_eq!(dominates.right_make_whole_atoms(), 3);
    }

    #[test]
    fn comparison_refuses_partition_substitution_and_unrepresentable_make_whole() {
        let first_partition = partition_three(4);
        let mut boundaries = [0i64; MAX_PARTITION_BOUNDARIES];
        boundaries[..4].copy_from_slice(&[-1, 1, 2, 3]);
        let mut substituted_representatives = [0i64; MAX_OUTCOMES];
        substituted_representatives[..3].copy_from_slice(&[0, 1, 3]);
        let substituted_partition = ExhaustivePartitionV1::new(
            domain(3),
            [4u8; 32],
            -1,
            3,
            boundaries,
            substituted_representatives,
        )
        .unwrap();
        let left = compile_payoff_shape_v1(
            first_partition,
            PayoffShapeV1::DigitalTail {
                direction: TailDirectionV1::LowerInclusive,
                threshold_point: 0,
                payout_atoms: 1,
            },
        )
        .unwrap();
        let foreign = compile_payoff_shape_v1(
            substituted_partition,
            PayoffShapeV1::DigitalTail {
                direction: TailDirectionV1::LowerInclusive,
                threshold_point: 0,
                payout_atoms: 1,
            },
        )
        .unwrap();
        assert_eq!(
            compare_compiled_payoffs_v1(left, 1, foreign, 1),
            Err(Error::MismatchedPartition)
        );
        assert_eq!(
            compare_compiled_payoffs_v1(left, 0, left, 1),
            Err(Error::ZeroPositionUnits)
        );

        let widest_left = compile_payoff_shape_v1(
            first_partition,
            PayoffShapeV1::DigitalTail {
                direction: TailDirectionV1::LowerInclusive,
                threshold_point: 0,
                payout_atoms: u64::MAX,
            },
        )
        .unwrap();
        let upper = compile_payoff_shape_v1(
            first_partition,
            PayoffShapeV1::DigitalTail {
                direction: TailDirectionV1::UpperInclusive,
                threshold_point: 1,
                payout_atoms: 1,
            },
        )
        .unwrap();
        assert_eq!(
            compare_compiled_payoffs_v1(widest_left, 1, upper, 1),
            Err(Error::ArithmeticOverflow)
        );
    }
}
