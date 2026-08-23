//! Exact inverse solver for singleton and two-atom quantized mixtures.
//!
//! The solver searches a caller-declared, canonical finite coordinate set. It
//! first checks singleton atoms, then coordinate pairs in lexicographic order.
//! For a pair it derives the unique primitive rational interpolation weight
//! directly from the first differing payout coordinate and verifies every
//! active payout equation. It never enumerates floating approximations or
//! rounds a residual into a certificate.

use crate::{
    verify_quantized_atom_mixture_v1, BoundQuantizedSplineV1, ErrorV1,
    QuantizedAtomMixtureCertificateV1, QuantizedPayoutPriceVectorV1,
    VerifiedQuantizedAtomMixtureV1, MAX_OUTCOMES, MAX_QUANTIZED_ATOMS,
};

/// Largest caller-declared coordinate set searched by the exact pair solver.
pub const MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1: usize = 64;

/// Canonical finite coordinate set searched by the inverse solver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomSearchCoordinatesV1 {
    coordinate_count: u8,
    coordinates: [u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1],
}

impl QuantizedAtomSearchCoordinatesV1 {
    /// Validate a nonempty strictly increasing prefix and zero padding.
    pub fn new(
        coordinate_count: u8,
        coordinates: [u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1],
    ) -> ResultPairSolverV1<Self> {
        let active = usize::from(coordinate_count);
        if active == 0 || active > MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1 {
            return Err(QuantizedAtomPairSolverErrorV1::InvalidCoordinateCount);
        }
        let mut coordinate = 0usize;
        while coordinate < MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1 {
            if coordinate < active {
                if coordinate != 0 && coordinates[coordinate] <= coordinates[coordinate - 1] {
                    return Err(
                        QuantizedAtomPairSolverErrorV1::NonCanonicalCoordinateOrder {
                            coordinate: u8_index(coordinate)?,
                        },
                    );
                }
            } else if coordinates[coordinate] != 0 {
                return Err(
                    QuantizedAtomPairSolverErrorV1::NonCanonicalCoordinatePadding {
                        coordinate: u8_index(coordinate)?,
                    },
                );
            }
            coordinate += 1;
        }
        Ok(Self {
            coordinate_count,
            coordinates,
        })
    }

    /// Active coordinate prefix length.
    pub const fn coordinate_count(&self) -> u8 {
        self.coordinate_count
    }

    /// Strictly increasing active coordinates followed by zero padding.
    pub const fn coordinates(
        &self,
    ) -> &[u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1] {
        &self.coordinates
    }
}

/// Explicit work bound for one deterministic inverse search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomPairSolverPlanV1 {
    maximum_pair_evaluations: u32,
}

impl QuantizedAtomPairSolverPlanV1 {
    /// Require a positive pair-evaluation budget.
    pub fn new(maximum_pair_evaluations: u32) -> ResultPairSolverV1<Self> {
        if maximum_pair_evaluations == 0 {
            return Err(QuantizedAtomPairSolverErrorV1::ZeroPairEvaluationLimit);
        }
        Ok(Self {
            maximum_pair_evaluations,
        })
    }

    /// Maximum lexicographic coordinate pairs evaluated after singleton search.
    pub const fn maximum_pair_evaluations(&self) -> u32 {
        self.maximum_pair_evaluations
    }
}

/// Factual coverage of one exact inverse search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomPairSolverReportV1 {
    coordinate_count: u8,
    singleton_evaluations: u8,
    pair_evaluations: u32,
    maximum_pair_evaluations: u32,
    covers_full_integer_domain: bool,
}

impl QuantizedAtomPairSolverReportV1 {
    /// Number of declared coordinates.
    pub const fn coordinate_count(&self) -> u8 {
        self.coordinate_count
    }

    /// Number of exact production atoms compared directly to the target.
    pub const fn singleton_evaluations(&self) -> u8 {
        self.singleton_evaluations
    }

    /// Number of lexicographic coordinate pairs considered.
    pub const fn pair_evaluations(&self) -> u32 {
        self.pair_evaluations
    }

    /// Caller-declared pair work bound.
    pub const fn maximum_pair_evaluations(&self) -> u32 {
        self.maximum_pair_evaluations
    }

    /// Whether the declared coordinates are every integer in the complete
    /// Terms domain, including both endpoints.
    pub const fn covers_full_integer_domain(&self) -> bool {
        self.covers_full_integer_domain
    }
}

/// One exact certificate constructed and independently reverified by the
/// production positive-mixture checker.
///
/// The contained identities repeat the supplied bound but do not authenticate
/// it. This is an authority-neutral arithmetic output until an adapter proves
/// the Market/Terms/Basis/price owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactQuantizedAtomSolutionV1 {
    certificate: QuantizedAtomMixtureCertificateV1,
    verified: VerifiedQuantizedAtomMixtureV1,
    report: QuantizedAtomPairSolverReportV1,
}

impl ExactQuantizedAtomSolutionV1 {
    /// Canonical sparse certificate body produced by the solver.
    pub const fn certificate(&self) -> &QuantizedAtomMixtureCertificateV1 {
        &self.certificate
    }

    /// Independent production-verifier result for the same certificate.
    pub const fn verified(&self) -> VerifiedQuantizedAtomMixtureV1 {
        self.verified
    }

    /// Exact search-prefix coverage that preceded the first solution.
    pub const fn report(&self) -> QuantizedAtomPairSolverReportV1 {
        self.report
    }
}

/// Total outcome of one bounded exact search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantizedAtomPairSolverOutcomeV1 {
    /// The first exact singleton or lexicographic pair was constructed and
    /// independently reverified.
    Solved(ExactQuantizedAtomSolutionV1),
    /// Every singleton and pair in the declared coordinate set was checked and
    /// none represented the target exactly.
    NoExactSingletonOrPairSolution(QuantizedAtomPairSolverReportV1),
    /// The pair budget ended before the declared coordinate set was exhausted.
    WorkLimitReached(QuantizedAtomPairSolverReportV1),
}

/// Malformed-input or checked-arithmetic refusals from the exact pair solver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantizedAtomPairSolverErrorV1 {
    /// The authoritative positive-mixture semantics refused an input or the
    /// solver's independently checked output.
    PriceMeasure(ErrorV1),
    /// The declared coordinate prefix was empty or exceeded 64.
    InvalidCoordinateCount,
    /// Active coordinates were not strictly increasing.
    NonCanonicalCoordinateOrder {
        /// First invalid coordinate index.
        coordinate: u8,
    },
    /// An inactive coordinate slot was nonzero.
    NonCanonicalCoordinatePadding {
        /// First nonzero padding index.
        coordinate: u8,
    },
    /// A declared coordinate was outside the complete Terms domain.
    CoordinateOutOfDomain {
        /// First out-of-domain coordinate index.
        coordinate: u8,
    },
    /// A pair search cannot have a zero work bound.
    ZeroPairEvaluationLimit,
    /// A checked integer conversion or counter overflowed.
    ArithmeticOverflow,
    /// Solver-derived facts disagreed despite exact input validation.
    InvariantViolation,
}

impl From<ErrorV1> for QuantizedAtomPairSolverErrorV1 {
    fn from(value: ErrorV1) -> Self {
        Self::PriceMeasure(value)
    }
}

/// Result alias for bounded exact pair-solver operations.
pub type ResultPairSolverV1<T> = core::result::Result<T, QuantizedAtomPairSolverErrorV1>;

/// Construct the first exact singleton or lexicographic two-atom certificate.
///
/// The target must already use the Basis payout denominator. The coordinate
/// set may be any canonical finite subset of the Terms domain. A negative
/// result is complete only for singleton and pair mixtures over that declared
/// set; [`QuantizedAtomPairSolverReportV1::covers_full_integer_domain`] states
/// separately whether the set covered the entire integer domain. There is no
/// claim about mixtures requiring three or more atoms and no optimality claim.
pub fn solve_quantized_atom_pair_hull_v1(
    bound: &BoundQuantizedSplineV1,
    prices: &QuantizedPayoutPriceVectorV1,
    coordinates: QuantizedAtomSearchCoordinatesV1,
    plan: QuantizedAtomPairSolverPlanV1,
) -> ResultPairSolverV1<QuantizedAtomPairSolverOutcomeV1> {
    let basis = bound.validated()?;
    let spec = basis.spec();
    validate_target_price(bound, prices, spec.outcome_count, spec.denominator)?;
    validate_coordinates(bound, coordinates)?;
    let mut report = QuantizedAtomPairSolverReportV1 {
        coordinate_count: coordinates.coordinate_count,
        singleton_evaluations: 0,
        pair_evaluations: 0,
        maximum_pair_evaluations: plan.maximum_pair_evaluations,
        covers_full_integer_domain: covers_full_integer_domain(bound, coordinates)?,
    };
    let active_coordinates = usize::from(coordinates.coordinate_count);
    let active_outcomes = usize::from(spec.outcome_count);

    let mut coordinate = 0usize;
    while coordinate < active_coordinates {
        let atom = basis
            .evaluate_point(coordinates.coordinates[coordinate])
            .map_err(|_| ErrorV1::AtomEvaluationFailed {
                witness: u8_index(coordinate)?,
            })?;
        report.singleton_evaluations = report
            .singleton_evaluations
            .checked_add(1)
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        if equal_active(&atom.weights, &prices.prices, active_outcomes) {
            return Ok(QuantizedAtomPairSolverOutcomeV1::Solved(
                make_solution(
                    bound,
                    prices,
                    coordinates.coordinates[coordinate],
                    0,
                    1,
                    0,
                    1,
                    report,
                )?,
            ));
        }
        coordinate += 1;
    }

    let mut left = 0usize;
    while left < active_coordinates {
        let left_atom = basis
            .evaluate_point(coordinates.coordinates[left])
            .map_err(|_| ErrorV1::AtomEvaluationFailed {
                witness: u8_index(left)?,
            })?;
        let mut right = left + 1;
        while right < active_coordinates {
            if report.pair_evaluations == plan.maximum_pair_evaluations {
                return Ok(QuantizedAtomPairSolverOutcomeV1::WorkLimitReached(
                    report,
                ));
            }
            report.pair_evaluations = report
                .pair_evaluations
                .checked_add(1)
                .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
            let right_atom = basis
                .evaluate_point(coordinates.coordinates[right])
                .map_err(|_| ErrorV1::AtomEvaluationFailed {
                    witness: u8_index(right)?,
                })?;
            if let Some((left_mass, right_mass, denominator)) = solve_pair_weights(
                &left_atom.weights,
                &right_atom.weights,
                &prices.prices,
                active_outcomes,
            )? {
                return Ok(QuantizedAtomPairSolverOutcomeV1::Solved(
                    make_solution(
                        bound,
                        prices,
                        coordinates.coordinates[left],
                        coordinates.coordinates[right],
                        left_mass,
                        right_mass,
                        denominator,
                        report,
                    )?,
                ));
            }
            right += 1;
        }
        left += 1;
    }

    Ok(
        QuantizedAtomPairSolverOutcomeV1::NoExactSingletonOrPairSolution(report),
    )
}

fn validate_target_price(
    bound: &BoundQuantizedSplineV1,
    prices: &QuantizedPayoutPriceVectorV1,
    outcome_count: u8,
    payout_denominator: u64,
) -> ResultPairSolverV1<()> {
    if prices.price_id == [0; 32]
        || prices.price_id != bound.bindings.price_id
        || prices.outcome_count != outcome_count
    {
        return Err(ErrorV1::PriceBindingMismatch.into());
    }
    let active = usize::from(outcome_count);
    let mut sum = 0u128;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        let price = prices.prices[outcome];
        if outcome < active {
            if price > payout_denominator {
                return Err(ErrorV1::PriceExceedsPayoutDenominator {
                    outcome: u8_index(outcome)?,
                }
                .into());
            }
            sum = sum
                .checked_add(u128::from(price))
                .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        } else if price != 0 {
            return Err(ErrorV1::NonCanonicalPricePadding {
                outcome: u8_index(outcome)?,
            }
            .into());
        }
        outcome += 1;
    }
    if sum != u128::from(payout_denominator) {
        return Err(ErrorV1::PriceSimplexMismatch.into());
    }
    Ok(())
}

fn validate_coordinates(
    bound: &BoundQuantizedSplineV1,
    coordinates: QuantizedAtomSearchCoordinatesV1,
) -> ResultPairSolverV1<()> {
    let mut coordinate = 0usize;
    while coordinate < usize::from(coordinates.coordinate_count) {
        let value = coordinates.coordinates[coordinate];
        if value < bound.coordinate_domain_min || value > bound.coordinate_domain_max {
            return Err(
                QuantizedAtomPairSolverErrorV1::CoordinateOutOfDomain {
                    coordinate: u8_index(coordinate)?,
                },
            );
        }
        coordinate += 1;
    }
    Ok(())
}

fn covers_full_integer_domain(
    bound: &BoundQuantizedSplineV1,
    coordinates: QuantizedAtomSearchCoordinatesV1,
) -> ResultPairSolverV1<bool> {
    let span = bound
        .coordinate_domain_max
        .checked_sub(bound.coordinate_domain_min)
        .ok_or(QuantizedAtomPairSolverErrorV1::InvariantViolation)?;
    let required = span
        .checked_add(1)
        .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
    let maximum_coordinate_count =
        u128::try_from(MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1)
            .map_err(|_| QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
    if required > maximum_coordinate_count
        || required != u128::from(coordinates.coordinate_count)
    {
        return Ok(false);
    }
    let mut coordinate = 0usize;
    while coordinate < usize::from(coordinates.coordinate_count) {
        let offset = u128::try_from(coordinate)
            .map_err(|_| QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let expected = bound
            .coordinate_domain_min
            .checked_add(offset)
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        if coordinates.coordinates[coordinate] != expected {
            return Ok(false);
        }
        coordinate += 1;
    }
    Ok(true)
}

fn solve_pair_weights(
    left: &[u64; MAX_OUTCOMES],
    right: &[u64; MAX_OUTCOMES],
    target: &[u64; MAX_OUTCOMES],
    active_outcomes: usize,
) -> ResultPairSolverV1<Option<(u64, u64, u64)>> {
    let mut pivot = None;
    let mut outcome = 0usize;
    while outcome < active_outcomes {
        if left[outcome] != right[outcome] {
            pivot = Some(outcome);
            break;
        }
        outcome += 1;
    }
    let Some(pivot) = pivot else {
        return Ok(None);
    };

    let (numerator, denominator) = if left[pivot] > right[pivot] {
        if target[pivot] <= right[pivot] || target[pivot] >= left[pivot] {
            return Ok(None);
        }
        (
            target[pivot] - right[pivot],
            left[pivot] - right[pivot],
        )
    } else {
        if target[pivot] >= right[pivot] || target[pivot] <= left[pivot] {
            return Ok(None);
        }
        (
            right[pivot] - target[pivot],
            right[pivot] - left[pivot],
        )
    };
    let divisor = gcd(numerator, denominator);
    let weight_denominator = denominator / divisor;
    let left_mass = numerator / divisor;
    let right_mass = weight_denominator
        .checked_sub(left_mass)
        .ok_or(QuantizedAtomPairSolverErrorV1::InvariantViolation)?;
    if left_mass == 0 || right_mass == 0 || gcd(weight_denominator, left_mass) != 1 {
        return Err(QuantizedAtomPairSolverErrorV1::InvariantViolation);
    }

    outcome = 0;
    while outcome < active_outcomes {
        let left_term = u128::from(left_mass)
            .checked_mul(u128::from(left[outcome]))
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let right_term = u128::from(right_mass)
            .checked_mul(u128::from(right[outcome]))
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let reconstructed = left_term
            .checked_add(right_term)
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let expected = u128::from(weight_denominator)
            .checked_mul(u128::from(target[outcome]))
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        if reconstructed != expected {
            return Ok(None);
        }
        outcome += 1;
    }
    Ok(Some((left_mass, right_mass, weight_denominator)))
}

#[allow(clippy::too_many_arguments)]
fn make_solution(
    bound: &BoundQuantizedSplineV1,
    prices: &QuantizedPayoutPriceVectorV1,
    left_coordinate: u128,
    right_coordinate: u128,
    left_mass: u64,
    right_mass: u64,
    weight_denominator: u64,
    report: QuantizedAtomPairSolverReportV1,
) -> ResultPairSolverV1<ExactQuantizedAtomSolutionV1> {
    let spec = bound.basis;
    let mut observation_coordinates = [0u128; MAX_QUANTIZED_ATOMS];
    let mut weights = [0u64; MAX_QUANTIZED_ATOMS];
    let witness_count = if right_mass == 0 {
        observation_coordinates[0] = left_coordinate;
        weights[0] = left_mass;
        1
    } else {
        observation_coordinates[0] = left_coordinate;
        observation_coordinates[1] = right_coordinate;
        weights[0] = left_mass;
        weights[1] = right_mass;
        2
    };
    let certificate = QuantizedAtomMixtureCertificateV1::new(
        bound.bindings,
        spec.degree,
        spec.outcome_count,
        spec.denominator,
        weight_denominator,
        witness_count,
        observation_coordinates,
        weights,
    )?;
    let verified = verify_quantized_atom_mixture_v1(bound, prices, &certificate)?;
    Ok(ExactQuantizedAtomSolutionV1 {
        certificate,
        verified,
        report,
    })
}

fn equal_active(
    left: &[u64; MAX_OUTCOMES],
    right: &[u64; MAX_OUTCOMES],
    active: usize,
) -> bool {
    let mut outcome = 0usize;
    while outcome < active {
        if left[outcome] != right[outcome] {
            return false;
        }
        outcome += 1;
    }
    true
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn u8_index(index: usize) -> ResultPairSolverV1<u8> {
    u8::try_from(index).map_err(|_| QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_bspline::{BasisSpec, EdgePolicy};
    use crate::QuantizedAtomMixtureBindingsV1;

    fn bound() -> BoundQuantizedSplineV1 {
        let mut knots = [0u128; 16];
        knots[..2].copy_from_slice(&[0, 4]);
        BoundQuantizedSplineV1 {
            bindings: QuantizedAtomMixtureBindingsV1 {
                market_id: [1; 32],
                terms_id: [2; 32],
                basis_id: [3; 32],
                price_id: [4; 32],
            },
            coordinate_domain_min: 0,
            coordinate_domain_max: 4,
            basis: BasisSpec {
                outcome_count: 3,
                degree: 2,
                knot_count: 2,
                uniform_log2_spacing: 2,
                denominator: 16,
                domain_max: 4,
                edge_policy: EdgePolicy::Refuse,
                knots,
            },
        }
    }

    fn target(prices: [u64; 3]) -> QuantizedPayoutPriceVectorV1 {
        let mut padded = [0u64; MAX_OUTCOMES];
        padded[..3].copy_from_slice(&prices);
        QuantizedPayoutPriceVectorV1 {
            price_id: [4; 32],
            outcome_count: 3,
            prices: padded,
        }
    }

    fn coordinates(values: &[u128]) -> QuantizedAtomSearchCoordinatesV1 {
        let mut padded = [0u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1];
        padded[..values.len()].copy_from_slice(values);
        QuantizedAtomSearchCoordinatesV1::new(u8::try_from(values.len()).unwrap(), padded).unwrap()
    }

    #[test]
    fn singleton_and_primitive_pair_are_constructed_and_reverified() {
        let bound = bound();
        let singleton = solve_quantized_atom_pair_hull_v1(
            &bound,
            &target([16, 0, 0]),
            coordinates(&[0, 2, 4]),
            QuantizedAtomPairSolverPlanV1::new(3).unwrap(),
        )
        .unwrap();
        let QuantizedAtomPairSolverOutcomeV1::Solved(singleton) = singleton else {
            panic!("expected singleton solution")
        };
        assert_eq!(singleton.certificate().witness_count, 1);
        assert_eq!(singleton.certificate().weight_denominator, 1);
        assert_eq!(singleton.certificate().observation_coordinates[0], 0);
        assert_eq!(singleton.certificate().weights[0], 1);

        let pair = solve_quantized_atom_pair_hull_v1(
            &bound,
            &target([10, 4, 2]),
            coordinates(&[0, 2, 4]),
            QuantizedAtomPairSolverPlanV1::new(3).unwrap(),
        )
        .unwrap();
        let QuantizedAtomPairSolverOutcomeV1::Solved(pair) = pair else {
            panic!("expected pair solution")
        };
        assert_eq!(pair.certificate().witness_count, 2);
        assert_eq!(pair.certificate().weight_denominator, 2);
        assert_eq!(&pair.certificate().observation_coordinates[..2], &[0, 2]);
        assert_eq!(&pair.certificate().weights[..2], &[1, 1]);
        assert_eq!(pair.verified().witness_count(), 2);
    }

    #[test]
    fn rational_pair_is_reduced_without_denominator_enumeration_or_rounding() {
        let solution = solve_quantized_atom_pair_hull_v1(
            &bound(),
            &target([4, 0, 12]),
            coordinates(&[0, 2, 4]),
            QuantizedAtomPairSolverPlanV1::new(3).unwrap(),
        )
        .unwrap();
        let QuantizedAtomPairSolverOutcomeV1::Solved(solution) = solution else {
            panic!("expected exact rational pair")
        };
        assert_eq!(solution.certificate().weight_denominator, 4);
        assert_eq!(&solution.certificate().weights[..2], &[1, 3]);
        assert_eq!(&solution.certificate().observation_coordinates[..2], &[0, 4]);
    }

    #[test]
    fn no_solution_and_work_limit_are_distinct_factual_outcomes() {
        let no_solution = solve_quantized_atom_pair_hull_v1(
            &bound(),
            &target([0, 16, 0]),
            coordinates(&[0, 2, 4]),
            QuantizedAtomPairSolverPlanV1::new(3).unwrap(),
        )
        .unwrap();
        let QuantizedAtomPairSolverOutcomeV1::NoExactSingletonOrPairSolution(report) =
            no_solution
        else {
            panic!("expected complete negative pair-hull result")
        };
        assert_eq!(report.singleton_evaluations(), 3);
        assert_eq!(report.pair_evaluations(), 3);

        let truncated = solve_quantized_atom_pair_hull_v1(
            &bound(),
            &target([4, 0, 12]),
            coordinates(&[0, 2, 4]),
            QuantizedAtomPairSolverPlanV1::new(1).unwrap(),
        )
        .unwrap();
        let QuantizedAtomPairSolverOutcomeV1::WorkLimitReached(report) = truncated else {
            panic!("expected explicit work-limit result")
        };
        assert_eq!(report.pair_evaluations(), 1);
        assert_eq!(report.maximum_pair_evaluations(), 1);
    }

    #[test]
    fn full_domain_claim_and_hostile_coordinate_shapes_are_exact() {
        let full = coordinates(&[0, 1, 2, 3, 4]);
        let result = solve_quantized_atom_pair_hull_v1(
            &bound(),
            &target([16, 0, 0]),
            full,
            QuantizedAtomPairSolverPlanV1::new(10).unwrap(),
        )
        .unwrap();
        let QuantizedAtomPairSolverOutcomeV1::Solved(solution) = result else {
            panic!("expected singleton")
        };
        assert!(solution.report().covers_full_integer_domain());

        let mut bad_order = [0u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1];
        bad_order[..3].copy_from_slice(&[0, 2, 2]);
        assert_eq!(
            QuantizedAtomSearchCoordinatesV1::new(3, bad_order),
            Err(QuantizedAtomPairSolverErrorV1::NonCanonicalCoordinateOrder {
                coordinate: 2,
            })
        );
        let mut bad_padding = [0u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1];
        bad_padding[0] = 0;
        bad_padding[1] = 9;
        assert_eq!(
            QuantizedAtomSearchCoordinatesV1::new(1, bad_padding),
            Err(QuantizedAtomPairSolverErrorV1::NonCanonicalCoordinatePadding {
                coordinate: 1,
            })
        );
        assert_eq!(
            solve_quantized_atom_pair_hull_v1(
                &bound(),
                &target([16, 0, 0]),
                coordinates(&[0, 5]),
                QuantizedAtomPairSolverPlanV1::new(1).unwrap(),
            ),
            Err(QuantizedAtomPairSolverErrorV1::CoordinateOutOfDomain {
                coordinate: 1,
            })
        );
    }

    #[test]
    fn malformed_target_and_zero_work_budget_refuse_before_search() {
        assert_eq!(
            QuantizedAtomPairSolverPlanV1::new(0),
            Err(QuantizedAtomPairSolverErrorV1::ZeroPairEvaluationLimit)
        );
        let mut malformed = target([16, 0, 0]);
        malformed.prices[3] = 1;
        assert_eq!(
            solve_quantized_atom_pair_hull_v1(
                &bound(),
                &malformed,
                coordinates(&[0, 2, 4]),
                QuantizedAtomPairSolverPlanV1::new(3).unwrap(),
            ),
            Err(QuantizedAtomPairSolverErrorV1::PriceMeasure(
                ErrorV1::NonCanonicalPricePadding { outcome: 3 },
            ))
        );
    }
}
