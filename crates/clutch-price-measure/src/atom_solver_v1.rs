//! Exact bounded inverse solvers for quantized atom mixtures.
//!
//! The solver searches a caller-declared, canonical finite coordinate set. It
//! checks supports in increasing size and lexicographic coordinate order. Pair
//! weights come directly from one exact interpolation equation; triple and
//! quartet weights use checked determinant arithmetic. Every candidate is
//! checked against all active payout equations, and every emitted certificate
//! is independently admitted by the production verifier. No inverse path
//! enumerates floating approximations or rounds a residual into a certificate.

use crate::{
    verify_quantized_atom_mixture_v1, BoundQuantizedSplineV1, ErrorV1,
    QuantizedAtomMixtureCertificateV1, QuantizedPayoutPriceVectorV1,
    VerifiedQuantizedAtomMixtureV1, MAX_OUTCOMES, MAX_QUANTIZED_ATOMS,
};
use crate::fraction_free_v1::{
    determinant_2x2, wide_gcd, FractionFreeErrorV1, FractionFreeMatrix3V1,
    SignedDeltaV1,
};
#[cfg(test)]
use crate::fraction_free_v1::WideUnsignedV1;

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

/// Explicit work bounds for deterministic support-at-most-three search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomSupport3SolverPlanV1 {
    maximum_pair_evaluations: u32,
    maximum_triple_evaluations: u32,
}

impl QuantizedAtomSupport3SolverPlanV1 {
    /// Require positive pair and triple evaluation budgets.
    pub fn new(
        maximum_pair_evaluations: u32,
        maximum_triple_evaluations: u32,
    ) -> ResultAtomSolverV1<Self> {
        if maximum_pair_evaluations == 0 {
            return Err(QuantizedAtomPairSolverErrorV1::ZeroPairEvaluationLimit);
        }
        if maximum_triple_evaluations == 0 {
            return Err(QuantizedAtomPairSolverErrorV1::ZeroTripleEvaluationLimit);
        }
        Ok(Self {
            maximum_pair_evaluations,
            maximum_triple_evaluations,
        })
    }

    /// Maximum lexicographic coordinate pairs evaluated after singleton search.
    pub const fn maximum_pair_evaluations(&self) -> u32 {
        self.maximum_pair_evaluations
    }

    /// Maximum lexicographic coordinate triples evaluated after pair search.
    pub const fn maximum_triple_evaluations(&self) -> u32 {
        self.maximum_triple_evaluations
    }
}

/// Explicit work bounds for deterministic support-at-most-four search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomSupport4SolverPlanV1 {
    maximum_pair_evaluations: u32,
    maximum_triple_evaluations: u32,
    maximum_quartet_evaluations: u32,
}

impl QuantizedAtomSupport4SolverPlanV1 {
    /// Require positive pair, triple, and quartet evaluation budgets.
    pub fn new(
        maximum_pair_evaluations: u32,
        maximum_triple_evaluations: u32,
        maximum_quartet_evaluations: u32,
    ) -> ResultAtomSolverV1<Self> {
        if maximum_pair_evaluations == 0 {
            return Err(QuantizedAtomPairSolverErrorV1::ZeroPairEvaluationLimit);
        }
        if maximum_triple_evaluations == 0 {
            return Err(QuantizedAtomPairSolverErrorV1::ZeroTripleEvaluationLimit);
        }
        if maximum_quartet_evaluations == 0 {
            return Err(QuantizedAtomPairSolverErrorV1::ZeroQuartetEvaluationLimit);
        }
        Ok(Self {
            maximum_pair_evaluations,
            maximum_triple_evaluations,
            maximum_quartet_evaluations,
        })
    }

    /// Maximum lexicographic coordinate pairs evaluated after singleton search.
    pub const fn maximum_pair_evaluations(&self) -> u32 {
        self.maximum_pair_evaluations
    }

    /// Maximum lexicographic coordinate triples evaluated after pair search.
    pub const fn maximum_triple_evaluations(&self) -> u32 {
        self.maximum_triple_evaluations
    }

    /// Maximum lexicographic coordinate quartets evaluated after triple search.
    pub const fn maximum_quartet_evaluations(&self) -> u32 {
        self.maximum_quartet_evaluations
    }
}

/// Factual coverage of one exact support-at-most-three inverse search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomSupport3SolverReportV1 {
    coordinate_count: u8,
    singleton_evaluations: u8,
    pair_evaluations: u32,
    triple_evaluations: u32,
    maximum_pair_evaluations: u32,
    maximum_triple_evaluations: u32,
    exact_but_unrepresentable_triples: u32,
    covers_full_integer_domain: bool,
}

impl QuantizedAtomSupport3SolverReportV1 {
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

    /// Number of lexicographic coordinate triples considered.
    pub const fn triple_evaluations(&self) -> u32 {
        self.triple_evaluations
    }

    /// Caller-declared pair work bound.
    pub const fn maximum_pair_evaluations(&self) -> u32 {
        self.maximum_pair_evaluations
    }

    /// Caller-declared triple work bound.
    pub const fn maximum_triple_evaluations(&self) -> u32 {
        self.maximum_triple_evaluations
    }

    /// Exact positive triple solutions whose primitive masses or denominator
    /// exceeded the V1 `u64` certificate profile.
    pub const fn exact_but_unrepresentable_triples(&self) -> u32 {
        self.exact_but_unrepresentable_triples
    }

    /// Whether the declared coordinates are every integer in the complete
    /// Terms domain, including both endpoints.
    pub const fn covers_full_integer_domain(&self) -> bool {
        self.covers_full_integer_domain
    }
}

/// Factual coverage of one exact support-at-most-four inverse search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomSupport4SolverReportV1 {
    coordinate_count: u8,
    singleton_evaluations: u8,
    pair_evaluations: u32,
    triple_evaluations: u32,
    quartet_evaluations: u32,
    maximum_pair_evaluations: u32,
    maximum_triple_evaluations: u32,
    maximum_quartet_evaluations: u32,
    exact_but_unrepresentable_triples: u32,
    exact_but_unrepresentable_quartets: u32,
    covers_full_integer_domain: bool,
}

impl QuantizedAtomSupport4SolverReportV1 {
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

    /// Number of lexicographic coordinate triples considered.
    pub const fn triple_evaluations(&self) -> u32 {
        self.triple_evaluations
    }

    /// Number of lexicographic coordinate quartets considered.
    pub const fn quartet_evaluations(&self) -> u32 {
        self.quartet_evaluations
    }

    /// Caller-declared pair work bound.
    pub const fn maximum_pair_evaluations(&self) -> u32 {
        self.maximum_pair_evaluations
    }

    /// Caller-declared triple work bound.
    pub const fn maximum_triple_evaluations(&self) -> u32 {
        self.maximum_triple_evaluations
    }

    /// Caller-declared quartet work bound.
    pub const fn maximum_quartet_evaluations(&self) -> u32 {
        self.maximum_quartet_evaluations
    }

    /// Exact positive triple solutions outside the V1 `u64` mass profile.
    pub const fn exact_but_unrepresentable_triples(&self) -> u32 {
        self.exact_but_unrepresentable_triples
    }

    /// Exact positive quartet solutions outside the V1 `u64` mass profile.
    pub const fn exact_but_unrepresentable_quartets(&self) -> u32 {
        self.exact_but_unrepresentable_quartets
    }

    /// Whether the declared coordinates are every integer in the complete
    /// Terms domain, including both endpoints.
    pub const fn covers_full_integer_domain(&self) -> bool {
        self.covers_full_integer_domain
    }
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

/// One support-at-most-three certificate constructed and independently
/// reverified by the production positive-mixture checker.
///
/// Repeated identities remain authority-neutral until an adapter proves the
/// owning Market/Terms/Basis/price bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactQuantizedSupport3SolutionV1 {
    certificate: QuantizedAtomMixtureCertificateV1,
    verified: VerifiedQuantizedAtomMixtureV1,
    report: QuantizedAtomSupport3SolverReportV1,
}

/// One support-at-most-four certificate constructed and independently
/// reverified by the production positive-mixture checker.
///
/// Repeated identities remain authority-neutral until an adapter proves the
/// owning Market/Terms/Basis/price bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactQuantizedSupport4SolutionV1 {
    certificate: QuantizedAtomMixtureCertificateV1,
    verified: VerifiedQuantizedAtomMixtureV1,
    report: QuantizedAtomSupport4SolverReportV1,
}

impl ExactQuantizedSupport4SolutionV1 {
    /// Canonical sparse certificate body produced by the solver.
    pub const fn certificate(&self) -> &QuantizedAtomMixtureCertificateV1 {
        &self.certificate
    }

    /// Independent production-verifier result for the same certificate.
    pub const fn verified(&self) -> VerifiedQuantizedAtomMixtureV1 {
        self.verified
    }

    /// Exact search-prefix coverage that preceded the first solution.
    pub const fn report(&self) -> QuantizedAtomSupport4SolverReportV1 {
        self.report
    }
}

impl ExactQuantizedSupport3SolutionV1 {
    /// Canonical sparse certificate body produced by the solver.
    pub const fn certificate(&self) -> &QuantizedAtomMixtureCertificateV1 {
        &self.certificate
    }

    /// Independent production-verifier result for the same certificate.
    pub const fn verified(&self) -> VerifiedQuantizedAtomMixtureV1 {
        self.verified
    }

    /// Exact search-prefix coverage that preceded the first solution.
    pub const fn report(&self) -> QuantizedAtomSupport3SolverReportV1 {
        self.report
    }
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

/// Total outcome of one bounded support-at-most-three exact search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantizedAtomSupport3SolverOutcomeV1 {
    /// The first exact singleton, pair, or lexicographic triple was constructed
    /// and independently reverified.
    Solved(ExactQuantizedSupport3SolutionV1),
    /// Every singleton, pair, and triple in the declared coordinate set was
    /// checked and none represented the target exactly.
    NoExactSingletonPairOrTripleSolution(QuantizedAtomSupport3SolverReportV1),
    /// At least one exact positive triple existed, but every such solution had
    /// a primitive mass or denominator outside the certificate's `u64` profile.
    ExactSolutionsExceedU64MassProfile(QuantizedAtomSupport3SolverReportV1),
    /// A pair or triple budget ended before its declared family was exhausted.
    WorkLimitReached(QuantizedAtomSupport3SolverReportV1),
}

/// Total outcome of one bounded support-at-most-four exact search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantizedAtomSupport4SolverOutcomeV1 {
    /// The first exact singleton, pair, triple, or lexicographic quartet was
    /// constructed and independently reverified.
    Solved(ExactQuantizedSupport4SolutionV1),
    /// The declared set was exhausted through support four without a
    /// representable certificate. This does not decide support above four or
    /// any coordinate omitted from the declared set.
    Unsupported(QuantizedAtomSupport4SolverReportV1),
    /// Exact positive solutions were seen, but their primitive masses or
    /// denominator exceeded the V1 certificate's `u64` representation.
    OutOfProfile(QuantizedAtomSupport4SolverReportV1),
    /// A pair, triple, or quartet budget ended before that family was exhausted.
    WorkTruncated(QuantizedAtomSupport4SolverReportV1),
}

/// Malformed-input or checked-arithmetic refusals from exact inverse solvers.
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
    /// A support-three search cannot have a zero triple work bound.
    ZeroTripleEvaluationLimit,
    /// A support-four search cannot have a zero quartet work bound.
    ZeroQuartetEvaluationLimit,
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

impl From<FractionFreeErrorV1> for QuantizedAtomPairSolverErrorV1 {
    fn from(value: FractionFreeErrorV1) -> Self {
        match value {
            FractionFreeErrorV1::ArithmeticOverflow => Self::ArithmeticOverflow,
            FractionFreeErrorV1::NonExactDivision
            | FractionFreeErrorV1::InvariantViolation => Self::InvariantViolation,
        }
    }
}

/// Result alias for bounded exact pair-solver operations.
pub type ResultPairSolverV1<T> = core::result::Result<T, QuantizedAtomPairSolverErrorV1>;

/// General name for the additive exact inverse atom-solver refusal set.
pub type QuantizedAtomSolverErrorV1 = QuantizedAtomPairSolverErrorV1;

/// Result alias shared by exact inverse atom-solver profiles.
pub type ResultAtomSolverV1<T> = core::result::Result<T, QuantizedAtomSolverErrorV1>;

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

/// Construct the first exact singleton, pair, or lexicographic triple
/// certificate over a declared finite coordinate set.
///
/// Pair search must finish before triple search begins. A work-limit result is
/// therefore factual about the visited prefix only. An exhaustive negative is
/// complete only for support of size at most three over the declared set. The
/// separate `ExactSolutionsExceedU64MassProfile` outcome means exact positive
/// rational triples existed, but their primitive certificate integers did not
/// fit the V1 `u64` mass profile. No outcome claims price incoherence for
/// support sizes four through `outcome_count`.
pub fn solve_quantized_atom_support3_hull_v1(
    bound: &BoundQuantizedSplineV1,
    prices: &QuantizedPayoutPriceVectorV1,
    coordinates: QuantizedAtomSearchCoordinatesV1,
    plan: QuantizedAtomSupport3SolverPlanV1,
) -> ResultAtomSolverV1<QuantizedAtomSupport3SolverOutcomeV1> {
    let basis = bound.validated()?;
    let spec = basis.spec();
    validate_target_price(bound, prices, spec.outcome_count, spec.denominator)?;
    validate_coordinates(bound, coordinates)?;
    let mut report = QuantizedAtomSupport3SolverReportV1 {
        coordinate_count: coordinates.coordinate_count,
        singleton_evaluations: 0,
        pair_evaluations: 0,
        triple_evaluations: 0,
        maximum_pair_evaluations: plan.maximum_pair_evaluations,
        maximum_triple_evaluations: plan.maximum_triple_evaluations,
        exact_but_unrepresentable_triples: 0,
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
            return Ok(QuantizedAtomSupport3SolverOutcomeV1::Solved(
                make_support3_solution(
                    bound,
                    prices,
                    [coordinates.coordinates[coordinate], 0, 0],
                    [1, 0, 0],
                    1,
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
                return Ok(QuantizedAtomSupport3SolverOutcomeV1::WorkLimitReached(
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
                return Ok(QuantizedAtomSupport3SolverOutcomeV1::Solved(
                    make_support3_solution(
                        bound,
                        prices,
                        [
                            coordinates.coordinates[left],
                            coordinates.coordinates[right],
                            0,
                        ],
                        [left_mass, right_mass, 0],
                        denominator,
                        2,
                        report,
                    )?,
                ));
            }
            right += 1;
        }
        left += 1;
    }

    left = 0;
    while left < active_coordinates {
        let left_atom = basis
            .evaluate_point(coordinates.coordinates[left])
            .map_err(|_| ErrorV1::AtomEvaluationFailed {
                witness: u8_index(left)?,
            })?;
        let mut middle = left + 1;
        while middle < active_coordinates {
            let middle_atom = basis
                .evaluate_point(coordinates.coordinates[middle])
                .map_err(|_| ErrorV1::AtomEvaluationFailed {
                    witness: u8_index(middle)?,
                })?;
            let mut right = middle + 1;
            while right < active_coordinates {
                if report.triple_evaluations == plan.maximum_triple_evaluations {
                    return Ok(QuantizedAtomSupport3SolverOutcomeV1::WorkLimitReached(
                        report,
                    ));
                }
                report.triple_evaluations = report
                    .triple_evaluations
                    .checked_add(1)
                    .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
                let right_atom = basis
                    .evaluate_point(coordinates.coordinates[right])
                    .map_err(|_| ErrorV1::AtomEvaluationFailed {
                        witness: u8_index(right)?,
                    })?;
                match solve_triple_weights(
                    &left_atom.weights,
                    &middle_atom.weights,
                    &right_atom.weights,
                    &prices.prices,
                    active_outcomes,
                )? {
                    TripleWeightSolutionV1::NoExactPositiveSolution => {}
                    TripleWeightSolutionV1::ExactButOutsideU64Profile => {
                        report.exact_but_unrepresentable_triples = report
                            .exact_but_unrepresentable_triples
                            .checked_add(1)
                            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
                    }
                    TripleWeightSolutionV1::Representable {
                        masses,
                        denominator,
                    } => {
                        return Ok(QuantizedAtomSupport3SolverOutcomeV1::Solved(
                            make_support3_solution(
                                bound,
                                prices,
                                [
                                    coordinates.coordinates[left],
                                    coordinates.coordinates[middle],
                                    coordinates.coordinates[right],
                                ],
                                masses,
                                denominator,
                                3,
                                report,
                            )?,
                        ));
                    }
                }
                right += 1;
            }
            middle += 1;
        }
        left += 1;
    }

    if report.exact_but_unrepresentable_triples != 0 {
        Ok(
            QuantizedAtomSupport3SolverOutcomeV1::ExactSolutionsExceedU64MassProfile(
                report,
            ),
        )
    } else {
        Ok(
            QuantizedAtomSupport3SolverOutcomeV1::NoExactSingletonPairOrTripleSolution(
                report,
            ),
        )
    }
}

/// Construct the first exact singleton, pair, triple, or lexicographic quartet
/// certificate over a declared finite coordinate set.
///
/// Each smaller support family must finish before the next begins. The caller
/// supplies a separate positive work bound for pairs, triples, and quartets.
/// `Unsupported` is deliberately a profile result: even after exhausting the
/// declared set through support four, this constructor makes no statement
/// about omitted coordinates or representations requiring support five through
/// `outcome_count`. `OutOfProfile` is separate from both mathematical absence
/// and work truncation. No result claims uniqueness, fair value, or optimality.
pub fn solve_quantized_atom_support4_hull_v1(
    bound: &BoundQuantizedSplineV1,
    prices: &QuantizedPayoutPriceVectorV1,
    coordinates: QuantizedAtomSearchCoordinatesV1,
    plan: QuantizedAtomSupport4SolverPlanV1,
) -> ResultAtomSolverV1<QuantizedAtomSupport4SolverOutcomeV1> {
    let basis = bound.validated()?;
    let spec = basis.spec();
    validate_target_price(bound, prices, spec.outcome_count, spec.denominator)?;
    validate_coordinates(bound, coordinates)?;
    let mut report = QuantizedAtomSupport4SolverReportV1 {
        coordinate_count: coordinates.coordinate_count,
        singleton_evaluations: 0,
        pair_evaluations: 0,
        triple_evaluations: 0,
        quartet_evaluations: 0,
        maximum_pair_evaluations: plan.maximum_pair_evaluations,
        maximum_triple_evaluations: plan.maximum_triple_evaluations,
        maximum_quartet_evaluations: plan.maximum_quartet_evaluations,
        exact_but_unrepresentable_triples: 0,
        exact_but_unrepresentable_quartets: 0,
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
            return Ok(QuantizedAtomSupport4SolverOutcomeV1::Solved(
                make_support4_solution(
                    bound,
                    prices,
                    [coordinates.coordinates[coordinate], 0, 0, 0],
                    [1, 0, 0, 0],
                    1,
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
                return Ok(QuantizedAtomSupport4SolverOutcomeV1::WorkTruncated(
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
                return Ok(QuantizedAtomSupport4SolverOutcomeV1::Solved(
                    make_support4_solution(
                        bound,
                        prices,
                        [
                            coordinates.coordinates[left],
                            coordinates.coordinates[right],
                            0,
                            0,
                        ],
                        [left_mass, right_mass, 0, 0],
                        denominator,
                        2,
                        report,
                    )?,
                ));
            }
            right += 1;
        }
        left += 1;
    }

    left = 0;
    while left < active_coordinates {
        let left_atom = basis
            .evaluate_point(coordinates.coordinates[left])
            .map_err(|_| ErrorV1::AtomEvaluationFailed {
                witness: u8_index(left)?,
            })?;
        let mut middle = left + 1;
        while middle < active_coordinates {
            let middle_atom = basis
                .evaluate_point(coordinates.coordinates[middle])
                .map_err(|_| ErrorV1::AtomEvaluationFailed {
                    witness: u8_index(middle)?,
                })?;
            let mut right = middle + 1;
            while right < active_coordinates {
                if report.triple_evaluations == plan.maximum_triple_evaluations {
                    return Ok(QuantizedAtomSupport4SolverOutcomeV1::WorkTruncated(
                        report,
                    ));
                }
                report.triple_evaluations = report
                    .triple_evaluations
                    .checked_add(1)
                    .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
                let right_atom = basis
                    .evaluate_point(coordinates.coordinates[right])
                    .map_err(|_| ErrorV1::AtomEvaluationFailed {
                        witness: u8_index(right)?,
                    })?;
                match solve_triple_weights(
                    &left_atom.weights,
                    &middle_atom.weights,
                    &right_atom.weights,
                    &prices.prices,
                    active_outcomes,
                )? {
                    TripleWeightSolutionV1::NoExactPositiveSolution => {}
                    TripleWeightSolutionV1::ExactButOutsideU64Profile => {
                        report.exact_but_unrepresentable_triples = report
                            .exact_but_unrepresentable_triples
                            .checked_add(1)
                            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
                    }
                    TripleWeightSolutionV1::Representable {
                        masses,
                        denominator,
                    } => {
                        return Ok(QuantizedAtomSupport4SolverOutcomeV1::Solved(
                            make_support4_solution(
                                bound,
                                prices,
                                [
                                    coordinates.coordinates[left],
                                    coordinates.coordinates[middle],
                                    coordinates.coordinates[right],
                                    0,
                                ],
                                [masses[0], masses[1], masses[2], 0],
                                denominator,
                                3,
                                report,
                            )?,
                        ));
                    }
                }
                right += 1;
            }
            middle += 1;
        }
        left += 1;
    }

    left = 0;
    while left < active_coordinates {
        let left_atom = basis
            .evaluate_point(coordinates.coordinates[left])
            .map_err(|_| ErrorV1::AtomEvaluationFailed {
                witness: u8_index(left)?,
            })?;
        let mut first_middle = left + 1;
        while first_middle < active_coordinates {
            let first_middle_atom = basis
                .evaluate_point(coordinates.coordinates[first_middle])
                .map_err(|_| ErrorV1::AtomEvaluationFailed {
                    witness: u8_index(first_middle)?,
                })?;
            let mut second_middle = first_middle + 1;
            while second_middle < active_coordinates {
                let second_middle_atom = basis
                    .evaluate_point(coordinates.coordinates[second_middle])
                    .map_err(|_| ErrorV1::AtomEvaluationFailed {
                        witness: u8_index(second_middle)?,
                    })?;
                let mut right = second_middle + 1;
                while right < active_coordinates {
                    if report.quartet_evaluations == plan.maximum_quartet_evaluations {
                        return Ok(QuantizedAtomSupport4SolverOutcomeV1::WorkTruncated(
                            report,
                        ));
                    }
                    report.quartet_evaluations = report
                        .quartet_evaluations
                        .checked_add(1)
                        .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
                    let right_atom = basis
                        .evaluate_point(coordinates.coordinates[right])
                        .map_err(|_| ErrorV1::AtomEvaluationFailed {
                            witness: u8_index(right)?,
                        })?;
                    match solve_quartet_weights(
                        [
                            &left_atom.weights,
                            &first_middle_atom.weights,
                            &second_middle_atom.weights,
                            &right_atom.weights,
                        ],
                        &prices.prices,
                        active_outcomes,
                    )? {
                        QuartetWeightSolutionV1::NoExactPositiveSolution => {}
                        QuartetWeightSolutionV1::ExactButOutsideU64Profile => {
                            report.exact_but_unrepresentable_quartets = report
                                .exact_but_unrepresentable_quartets
                                .checked_add(1)
                                .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
                        }
                        QuartetWeightSolutionV1::Representable {
                            masses,
                            denominator,
                        } => {
                            return Ok(QuantizedAtomSupport4SolverOutcomeV1::Solved(
                                make_support4_solution(
                                    bound,
                                    prices,
                                    [
                                        coordinates.coordinates[left],
                                        coordinates.coordinates[first_middle],
                                        coordinates.coordinates[second_middle],
                                        coordinates.coordinates[right],
                                    ],
                                    masses,
                                    denominator,
                                    4,
                                    report,
                                )?,
                            ));
                        }
                    }
                    right += 1;
                }
                second_middle += 1;
            }
            first_middle += 1;
        }
        left += 1;
    }

    if report.exact_but_unrepresentable_triples != 0
        || report.exact_but_unrepresentable_quartets != 0
    {
        Ok(QuantizedAtomSupport4SolverOutcomeV1::OutOfProfile(report))
    } else {
        Ok(QuantizedAtomSupport4SolverOutcomeV1::Unsupported(report))
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TripleWeightSolutionV1 {
    NoExactPositiveSolution,
    ExactButOutsideU64Profile,
    Representable {
        masses: [u64; 3],
        denominator: u64,
    },
}

fn solve_triple_weights(
    left: &[u64; MAX_OUTCOMES],
    middle: &[u64; MAX_OUTCOMES],
    right: &[u64; MAX_OUTCOMES],
    target: &[u64; MAX_OUTCOMES],
    active_outcomes: usize,
) -> ResultAtomSolverV1<TripleWeightSolutionV1> {
    let mut pivot = None;
    let mut first = 0usize;
    while first < active_outcomes {
        let left_first = SignedDeltaV1::between(left[first], right[first]);
        let middle_first = SignedDeltaV1::between(middle[first], right[first]);
        let mut second = first + 1;
        while second < active_outcomes {
            let determinant = determinant_2x2(
                left_first,
                SignedDeltaV1::between(middle[second], right[second]),
                middle_first,
                SignedDeltaV1::between(left[second], right[second]),
            )?;
            if !determinant.magnitude.is_zero() {
                pivot = Some((first, second, determinant));
                break;
            }
            second += 1;
        }
        if pivot.is_some() {
            break;
        }
        first += 1;
    }
    let Some((first, second, mut denominator)) = pivot else {
        return Ok(TripleWeightSolutionV1::NoExactPositiveSolution);
    };

    let target_first = SignedDeltaV1::between(target[first], right[first]);
    let target_second = SignedDeltaV1::between(target[second], right[second]);
    let left_first = SignedDeltaV1::between(left[first], right[first]);
    let left_second = SignedDeltaV1::between(left[second], right[second]);
    let middle_first = SignedDeltaV1::between(middle[first], right[first]);
    let middle_second = SignedDeltaV1::between(middle[second], right[second]);
    let mut left_numerator = determinant_2x2(
        target_first,
        middle_second,
        middle_first,
        target_second,
    )?;
    let mut middle_numerator = determinant_2x2(
        left_first,
        target_second,
        target_first,
        left_second,
    )?;
    if denominator.negative {
        denominator = denominator.negated();
        left_numerator = left_numerator.negated();
        middle_numerator = middle_numerator.negated();
    }
    if left_numerator.negative
        || middle_numerator.negative
        || left_numerator.magnitude.is_zero()
        || middle_numerator.magnitude.is_zero()
    {
        return Ok(TripleWeightSolutionV1::NoExactPositiveSolution);
    }
    let first_two = left_numerator
        .magnitude
        .checked_add(middle_numerator.magnitude)
        .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
    if first_two >= denominator.magnitude {
        return Ok(TripleWeightSolutionV1::NoExactPositiveSolution);
    }
    let right_numerator = denominator
        .magnitude
        .checked_sub(first_two)
        .ok_or(QuantizedAtomPairSolverErrorV1::InvariantViolation)?;

    let mut outcome = 0usize;
    while outcome < active_outcomes {
        let left_term = left_numerator
            .magnitude
            .checked_mul_u64(left[outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let middle_term = middle_numerator
            .magnitude
            .checked_mul_u64(middle[outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let right_term = right_numerator
            .checked_mul_u64(right[outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let reconstructed = left_term
            .checked_add(middle_term)
            .and_then(|sum| sum.checked_add(right_term))
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let expected = denominator
            .magnitude
            .checked_mul_u64(target[outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        if reconstructed != expected {
            return Ok(TripleWeightSolutionV1::NoExactPositiveSolution);
        }
        outcome += 1;
    }

    let divisor = wide_gcd(
        wide_gcd(
            wide_gcd(denominator.magnitude, left_numerator.magnitude)?,
            middle_numerator.magnitude,
        )?,
        right_numerator,
    )?;
    let reduced_denominator = denominator.magnitude.checked_div_exact(divisor)?;
    let reduced_left = left_numerator.magnitude.checked_div_exact(divisor)?;
    let reduced_middle = middle_numerator.magnitude.checked_div_exact(divisor)?;
    let reduced_right = right_numerator.checked_div_exact(divisor)?;
    let (Some(denominator), Some(left_mass), Some(middle_mass), Some(right_mass)) = (
        reduced_denominator.to_u64(),
        reduced_left.to_u64(),
        reduced_middle.to_u64(),
        reduced_right.to_u64(),
    ) else {
        return Ok(TripleWeightSolutionV1::ExactButOutsideU64Profile);
    };
    let mass_sum = left_mass
        .checked_add(middle_mass)
        .and_then(|sum| sum.checked_add(right_mass))
        .ok_or(QuantizedAtomPairSolverErrorV1::InvariantViolation)?;
    if left_mass == 0
        || middle_mass == 0
        || right_mass == 0
        || mass_sum != denominator
    {
        return Err(QuantizedAtomPairSolverErrorV1::InvariantViolation);
    }
    Ok(TripleWeightSolutionV1::Representable {
        masses: [left_mass, middle_mass, right_mass],
        denominator,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QuartetWeightSolutionV1 {
    NoExactPositiveSolution,
    ExactButOutsideU64Profile,
    Representable {
        masses: [u64; 4],
        denominator: u64,
    },
}

fn solve_quartet_weights(
    atoms: [&[u64; MAX_OUTCOMES]; 4],
    target: &[u64; MAX_OUTCOMES],
    active_outcomes: usize,
) -> ResultAtomSolverV1<QuartetWeightSolutionV1> {
    let mut pivot = None;
    let mut first = 0usize;
    while first < active_outcomes {
        let mut second = first + 1;
        while second < active_outcomes {
            let mut third = second + 1;
            while third < active_outcomes {
                let rows = quartet_difference_rows(atoms, [first, second, third]);
                let matrix = FractionFreeMatrix3V1::new(rows);
                let determinant = matrix.determinant()?;
                if !determinant.magnitude.is_zero() {
                    pivot = Some((matrix, determinant, [first, second, third]));
                    break;
                }
                third += 1;
            }
            if pivot.is_some() {
                break;
            }
            second += 1;
        }
        if pivot.is_some() {
            break;
        }
        first += 1;
    }
    let Some((matrix, mut denominator, pivot_rows)) = pivot else {
        // An affinely dependent quartet has no irreducible support-four point:
        // any positive representation can be moved to a boundary support that
        // the already-exhausted triple search owns.
        return Ok(QuartetWeightSolutionV1::NoExactPositiveSolution);
    };
    let right_hand_side = [
        SignedDeltaV1::between(target[pivot_rows[0]], atoms[3][pivot_rows[0]]),
        SignedDeltaV1::between(target[pivot_rows[1]], atoms[3][pivot_rows[1]]),
        SignedDeltaV1::between(target[pivot_rows[2]], atoms[3][pivot_rows[2]]),
    ];
    let mut numerators = [
        matrix.with_column(0, right_hand_side)?.determinant()?,
        matrix.with_column(1, right_hand_side)?.determinant()?,
        matrix.with_column(2, right_hand_side)?.determinant()?,
    ];
    if denominator.negative {
        denominator = denominator.negated();
        let mut numerator = 0usize;
        while numerator < 3 {
            numerators[numerator] = numerators[numerator].negated();
            numerator += 1;
        }
    }
    let mut numerator = 0usize;
    while numerator < 3 {
        if numerators[numerator].negative || numerators[numerator].magnitude.is_zero() {
            return Ok(QuartetWeightSolutionV1::NoExactPositiveSolution);
        }
        numerator += 1;
    }
    let first_three = numerators[0]
        .magnitude
        .checked_add(numerators[1].magnitude)
        .and_then(|sum| sum.checked_add(numerators[2].magnitude))
        .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
    if first_three >= denominator.magnitude {
        return Ok(QuartetWeightSolutionV1::NoExactPositiveSolution);
    }
    let fourth_numerator = denominator
        .magnitude
        .checked_sub(first_three)
        .ok_or(QuantizedAtomPairSolverErrorV1::InvariantViolation)?;

    let mut outcome = 0usize;
    while outcome < active_outcomes {
        let first_term = numerators[0]
            .magnitude
            .checked_mul_u64(atoms[0][outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let second_term = numerators[1]
            .magnitude
            .checked_mul_u64(atoms[1][outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let third_term = numerators[2]
            .magnitude
            .checked_mul_u64(atoms[2][outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let fourth_term = fourth_numerator
            .checked_mul_u64(atoms[3][outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let reconstructed = first_term
            .checked_add(second_term)
            .and_then(|sum| sum.checked_add(third_term))
            .and_then(|sum| sum.checked_add(fourth_term))
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        let expected = denominator
            .magnitude
            .checked_mul_u64(target[outcome])
            .ok_or(QuantizedAtomPairSolverErrorV1::ArithmeticOverflow)?;
        if reconstructed != expected {
            return Ok(QuartetWeightSolutionV1::NoExactPositiveSolution);
        }
        outcome += 1;
    }

    let mut divisor = denominator.magnitude;
    numerator = 0;
    while numerator < 3 {
        divisor = wide_gcd(divisor, numerators[numerator].magnitude)?;
        numerator += 1;
    }
    divisor = wide_gcd(divisor, fourth_numerator)?;
    let reduced_denominator = denominator.magnitude.checked_div_exact(divisor)?;
    let reduced_first = numerators[0].magnitude.checked_div_exact(divisor)?;
    let reduced_second = numerators[1].magnitude.checked_div_exact(divisor)?;
    let reduced_third = numerators[2].magnitude.checked_div_exact(divisor)?;
    let reduced_fourth = fourth_numerator.checked_div_exact(divisor)?;
    let (
        Some(denominator),
        Some(first_mass),
        Some(second_mass),
        Some(third_mass),
        Some(fourth_mass),
    ) = (
        reduced_denominator.to_u64(),
        reduced_first.to_u64(),
        reduced_second.to_u64(),
        reduced_third.to_u64(),
        reduced_fourth.to_u64(),
    )
    else {
        return Ok(QuartetWeightSolutionV1::ExactButOutsideU64Profile);
    };
    let mass_sum = first_mass
        .checked_add(second_mass)
        .and_then(|sum| sum.checked_add(third_mass))
        .and_then(|sum| sum.checked_add(fourth_mass))
        .ok_or(QuantizedAtomPairSolverErrorV1::InvariantViolation)?;
    if first_mass == 0
        || second_mass == 0
        || third_mass == 0
        || fourth_mass == 0
        || mass_sum != denominator
    {
        return Err(QuantizedAtomPairSolverErrorV1::InvariantViolation);
    }
    Ok(QuartetWeightSolutionV1::Representable {
        masses: [first_mass, second_mass, third_mass, fourth_mass],
        denominator,
    })
}

fn quartet_difference_rows(
    atoms: [&[u64; MAX_OUTCOMES]; 4],
    outcomes: [usize; 3],
) -> [[SignedDeltaV1; 3]; 3] {
    let mut rows = [[SignedDeltaV1::between(0, 0); 3]; 3];
    let mut row = 0usize;
    while row < 3 {
        let outcome = outcomes[row];
        let mut column = 0usize;
        while column < 3 {
            rows[row][column] =
                SignedDeltaV1::between(atoms[column][outcome], atoms[3][outcome]);
            column += 1;
        }
        row += 1;
    }
    rows
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

#[allow(clippy::too_many_arguments)]
fn make_support3_solution(
    bound: &BoundQuantizedSplineV1,
    prices: &QuantizedPayoutPriceVectorV1,
    coordinates: [u128; 3],
    masses: [u64; 3],
    weight_denominator: u64,
    witness_count: u8,
    report: QuantizedAtomSupport3SolverReportV1,
) -> ResultAtomSolverV1<ExactQuantizedSupport3SolutionV1> {
    let spec = bound.basis;
    let mut observation_coordinates = [0u128; MAX_QUANTIZED_ATOMS];
    let mut weights = [0u64; MAX_QUANTIZED_ATOMS];
    let mut witness = 0usize;
    while witness < usize::from(witness_count) {
        observation_coordinates[witness] = coordinates[witness];
        weights[witness] = masses[witness];
        witness += 1;
    }
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
    Ok(ExactQuantizedSupport3SolutionV1 {
        certificate,
        verified,
        report,
    })
}

#[allow(clippy::too_many_arguments)]
fn make_support4_solution(
    bound: &BoundQuantizedSplineV1,
    prices: &QuantizedPayoutPriceVectorV1,
    coordinates: [u128; 4],
    masses: [u64; 4],
    weight_denominator: u64,
    witness_count: u8,
    report: QuantizedAtomSupport4SolverReportV1,
) -> ResultAtomSolverV1<ExactQuantizedSupport4SolutionV1> {
    let spec = bound.basis;
    let mut observation_coordinates = [0u128; MAX_QUANTIZED_ATOMS];
    let mut weights = [0u64; MAX_QUANTIZED_ATOMS];
    let mut witness = 0usize;
    while witness < usize::from(witness_count) {
        observation_coordinates[witness] = coordinates[witness];
        weights[witness] = masses[witness];
        witness += 1;
    }
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
    Ok(ExactQuantizedSupport4SolutionV1 {
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

    fn support4_bound() -> BoundQuantizedSplineV1 {
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
                outcome_count: 4,
                degree: 3,
                knot_count: 2,
                uniform_log2_spacing: 2,
                denominator: 64,
                domain_max: 4,
                edge_policy: EdgePolicy::Refuse,
                knots,
            },
        }
    }

    fn support4_target(prices: [u64; 4]) -> QuantizedPayoutPriceVectorV1 {
        let mut padded = [0u64; MAX_OUTCOMES];
        padded[..4].copy_from_slice(&prices);
        QuantizedPayoutPriceVectorV1 {
            price_id: [4; 32],
            outcome_count: 4,
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
    fn exact_interior_triple_is_constructed_reduced_and_reverified() {
        let solution = solve_quantized_atom_support3_hull_v1(
            &bound(),
            &target([6, 4, 6]),
            coordinates(&[0, 2, 4]),
            QuantizedAtomSupport3SolverPlanV1::new(3, 1).unwrap(),
        )
        .unwrap();
        let QuantizedAtomSupport3SolverOutcomeV1::Solved(solution) = solution else {
            panic!("expected exact support-three solution")
        };
        assert_eq!(solution.certificate().witness_count, 3);
        assert_eq!(solution.certificate().weight_denominator, 4);
        assert_eq!(
            &solution.certificate().observation_coordinates[..3],
            &[0, 2, 4]
        );
        assert_eq!(&solution.certificate().weights[..3], &[1, 2, 1]);
        assert_eq!(solution.verified().witness_count(), 3);
        assert_eq!(solution.report().pair_evaluations(), 3);
        assert_eq!(solution.report().triple_evaluations(), 1);
    }

    #[test]
    fn support_three_negative_and_truncated_searches_are_distinct() {
        let negative = solve_quantized_atom_support3_hull_v1(
            &bound(),
            &target([0, 16, 0]),
            coordinates(&[0, 2, 4]),
            QuantizedAtomSupport3SolverPlanV1::new(3, 1).unwrap(),
        )
        .unwrap();
        let QuantizedAtomSupport3SolverOutcomeV1::NoExactSingletonPairOrTripleSolution(
            report,
        ) = negative
        else {
            panic!("expected exhaustive support-three negative")
        };
        assert_eq!(report.singleton_evaluations(), 3);
        assert_eq!(report.pair_evaluations(), 3);
        assert_eq!(report.triple_evaluations(), 1);
        assert_eq!(report.exact_but_unrepresentable_triples(), 0);

        let pair_truncated = solve_quantized_atom_support3_hull_v1(
            &bound(),
            &target([6, 4, 6]),
            coordinates(&[0, 2, 4]),
            QuantizedAtomSupport3SolverPlanV1::new(1, 1).unwrap(),
        )
        .unwrap();
        let QuantizedAtomSupport3SolverOutcomeV1::WorkLimitReached(report) = pair_truncated
        else {
            panic!("expected pair-prefix work limit")
        };
        assert_eq!(report.pair_evaluations(), 1);
        assert_eq!(report.triple_evaluations(), 0);

        let triple_truncated = solve_quantized_atom_support3_hull_v1(
            &bound(),
            &target([0, 16, 0]),
            coordinates(&[0, 1, 2, 4]),
            QuantizedAtomSupport3SolverPlanV1::new(6, 1).unwrap(),
        )
        .unwrap();
        let QuantizedAtomSupport3SolverOutcomeV1::WorkLimitReached(report) = triple_truncated
        else {
            panic!("expected triple-prefix work limit")
        };
        assert_eq!(report.pair_evaluations(), 6);
        assert_eq!(report.triple_evaluations(), 1);
    }

    #[test]
    fn exact_triple_outside_u64_mass_profile_is_not_a_negative() {
        let denominator = u64::MAX;
        let mut left = [0u64; MAX_OUTCOMES];
        let mut middle = [0u64; MAX_OUTCOMES];
        let mut right = [0u64; MAX_OUTCOMES];
        let mut target = [0u64; MAX_OUTCOMES];
        left[..3].copy_from_slice(&[denominator - 1, 1, 0]);
        middle[..3].copy_from_slice(&[1, denominator - 1, 0]);
        right[..3].copy_from_slice(&[0, 0, denominator]);
        target[..3].copy_from_slice(&[1, 2, denominator - 3]);
        assert_eq!(
            solve_triple_weights(&left, &middle, &right, &target, 3).unwrap(),
            TripleWeightSolutionV1::ExactButOutsideU64Profile
        );
    }

    #[test]
    fn exact_interior_quartet_is_constructed_reduced_and_reverified() {
        let solution = solve_quantized_atom_support4_hull_v1(
            &support4_bound(),
            &support4_target([49, 5, 3, 7]),
            coordinates(&[0, 1, 2, 4]),
            QuantizedAtomSupport4SolverPlanV1::new(6, 4, 1).unwrap(),
        )
        .unwrap();
        let QuantizedAtomSupport4SolverOutcomeV1::Solved(solution) = solution else {
            panic!("expected exact support-four solution")
        };
        assert_eq!(solution.certificate().witness_count, 4);
        assert_eq!(solution.certificate().weight_denominator, 72);
        assert_eq!(
            &solution.certificate().observation_coordinates[..4],
            &[0, 1, 2, 4]
        );
        assert_eq!(&solution.certificate().weights[..4], &[51, 8, 6, 7]);
        assert_eq!(solution.verified().witness_count(), 4);
        assert_eq!(solution.report().pair_evaluations(), 6);
        assert_eq!(solution.report().triple_evaluations(), 4);
        assert_eq!(solution.report().quartet_evaluations(), 1);
    }

    #[test]
    fn support_four_unsupported_work_truncated_and_out_of_profile_are_distinct() {
        let unsupported = solve_quantized_atom_support4_hull_v1(
            &support4_bound(),
            &support4_target([0, 64, 0, 0]),
            coordinates(&[0, 1, 2, 4]),
            QuantizedAtomSupport4SolverPlanV1::new(6, 4, 1).unwrap(),
        )
        .unwrap();
        let QuantizedAtomSupport4SolverOutcomeV1::Unsupported(report) = unsupported else {
            panic!("expected support-four profile exhaustion")
        };
        assert_eq!(report.quartet_evaluations(), 1);

        let truncated = solve_quantized_atom_support4_hull_v1(
            &support4_bound(),
            &support4_target([0, 64, 0, 0]),
            coordinates(&[0, 1, 2, 3, 4]),
            QuantizedAtomSupport4SolverPlanV1::new(10, 10, 1).unwrap(),
        )
        .unwrap();
        let QuantizedAtomSupport4SolverOutcomeV1::WorkTruncated(report) = truncated else {
            panic!("expected quartet-prefix work truncation")
        };
        assert_eq!(report.quartet_evaluations(), 1);
        assert_eq!(report.maximum_quartet_evaluations(), 1);

        let denominator = u64::MAX;
        let mut atoms = [[0u64; MAX_OUTCOMES]; 4];
        atoms[0][..4].copy_from_slice(&[denominator - 1, 1, 0, 0]);
        atoms[1][..4].copy_from_slice(&[1, denominator - 1, 0, 0]);
        atoms[2][..4].copy_from_slice(&[0, 0, denominator, 0]);
        atoms[3][..4].copy_from_slice(&[0, 0, 0, denominator]);
        let mut target = [0u64; MAX_OUTCOMES];
        target[..4].copy_from_slice(&[1, 2, 1, denominator - 4]);
        assert_eq!(
            solve_quartet_weights(
                [&atoms[0], &atoms[1], &atoms[2], &atoms[3]],
                &target,
                4,
            )
            .unwrap(),
            QuartetWeightSolutionV1::ExactButOutsideU64Profile,
        );
    }

    #[test]
    fn wide_determinant_gcd_and_exact_division_preserve_the_high_limb() {
        let positive = SignedDeltaV1 {
            negative: false,
            magnitude: u64::MAX,
        };
        let negative = SignedDeltaV1 {
            negative: true,
            magnitude: u64::MAX,
        };
        let determinant = determinant_2x2(positive, positive, negative, positive).unwrap();
        assert!(!determinant.negative);
        assert_eq!(determinant.magnitude.limb(2), Some(1));

        let divisor = WideUnsignedV1::from_u128(1u128 << 127).unwrap();
        let dividend = divisor.checked_mul_u64(2).unwrap();
        assert_eq!(wide_gcd(dividend, divisor).unwrap(), divisor);
        assert_eq!(
            dividend.checked_div_exact(divisor).unwrap(),
            WideUnsignedV1::from_u128(2).unwrap()
        );
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
        assert_eq!(
            QuantizedAtomSupport3SolverPlanV1::new(1, 0),
            Err(QuantizedAtomPairSolverErrorV1::ZeroTripleEvaluationLimit)
        );
        assert_eq!(
            QuantizedAtomSupport4SolverPlanV1::new(1, 1, 0),
            Err(QuantizedAtomPairSolverErrorV1::ZeroQuartetEvaluationLimit)
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
