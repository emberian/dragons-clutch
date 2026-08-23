//! Exact-rational offline compiler for bounded shapes over native B-spline Eggs.
//!
//! This crate is research tooling, not consensus code.  It uses arbitrary-size
//! rationals to distinguish an exact spline-span representation from a
//! certified approximation.  Consensus continues to use the float-free,
//! allocation-free `clutch-bspline` evaluator.

use clutch_bspline::{BasisSpec, EdgePolicy, MAX_OUTCOMES};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// Canonical native-basis, compiler-certificate, and market-intent artifacts.
pub mod artifact;
/// Untrusted canonical Product-facing basis, payoff, and bundle target.
pub mod production;
/// Exact bridge from compiler coefficients to the live portfolio identity and
/// a transferable, complete-set-compressed backing plan.
pub mod wrapper;

/// Fixed exact-rational interval-subdivision depth.
pub const CERTIFICATION_DEPTH: u8 = 8;

/// Maximum exact range-reduction squarings used for one exponential sample.
///
/// Beyond this point the size of an exact rational would grow exponentially.
/// The compiler switches to the proof-valid enclosure
/// `0 <= exp(-z) <= 1 / (1 + z)` instead.
pub const MAX_EXP_RANGE_SQUARINGS: u8 = 4;

/// Exponent in the far-tail rational Gaussian enclosure.
pub const EXP_FAR_TAIL_POWER: u8 = 32;

/// Compiler refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidBasis,
    InvalidShape,
    UnsupportedEdgePolicy,
    NotCategorical,
    DomainMismatch,
    TooManyOutcomes,
    ArithmeticConversion,
    InternalInvariant,
}

/// Whether the emitted coefficients define the requested shape exactly in the
/// selected native span or only an approximation with a certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpanStatus {
    ExactInSpan,
    CertifiedApproximation,
}

/// Named construction used for the coefficient vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Construction {
    DegreeZeroCells,
    DegreeOneInterpolation,
    GrevilleAffineReproduction,
    SchoenbergGrevilleQuasiInterpolant,
    GaussianIntervalSamples,
}

/// Supported bounded analytic shapes.  Coordinates and heights are exact
/// integers; evaluation is rational.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    /// `height` on `[low, high)`, zero elsewhere.
    HardRange { low: u128, high: u128, height: u64 },
    /// `height` on `[strike, +infinity)`, zero below.
    UpperTail { strike: u128, height: u64 },
    /// `height` below `strike`, zero on `[strike, +infinity)`.
    LowerTail { strike: u128, height: u64 },
    /// Piecewise-linear tent through `(left,0)`, `(peak,height)`, `(right,0)`.
    Triangle {
        left: u128,
        peak: u128,
        right: u128,
        height: u64,
    },
    /// Capped call/call-spread ramp: zero through `low`, linear, then `height`.
    CappedCall { low: u128, high: u128, height: u64 },
    /// Capped put/put-spread ramp: `height` through `low`, linear, then zero.
    CappedPut { low: u128, high: u128, height: u64 },
    /// Gaussian proximity kernel `height * exp(-(x-center)^2/(2 sigma^2))`.
    Gaussian {
        center: u128,
        sigma: u128,
        height: u64,
    },
}

/// Closed rational error enclosure.  `l1` is over the unclamped native knot
/// span (or the full degree-zero domain) in coordinate units.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorCertificate {
    pub spline_sup_lower: BigRational,
    pub spline_sup_upper: BigRational,
    pub spline_l1_lower: BigRational,
    pub spline_l1_upper: BigRational,
    /// Extra conservative sup error from `WEIGHT-ROUND-01` at denominator `D`.
    pub consensus_quantization_sup_upper: BigRational,
    /// Total target-versus-consensus upper bound, capped by `height`.
    pub consensus_sup_upper: BigRational,
    pub consensus_l1_upper: BigRational,
    /// Maximum error in any Gaussian sample coefficient enclosure.
    pub coefficient_sample_sup_upper: BigRational,
    pub subdivision_depth: u8,
}

/// One exact-rational coefficient artifact and its approximation claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compilation {
    pub status: SpanStatus,
    pub construction: Construction,
    pub coefficients: Vec<BigRational>,
    pub height: BigRational,
    pub max_coefficient: BigRational,
    pub certificate: ErrorCertificate,
}

/// Rational lower and upper bounds for sup and L1 norms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormBounds {
    pub sup_lower: BigRational,
    pub sup_upper: BigRational,
    pub l1_lower: BigRational,
    pub l1_upper: BigRational,
}

/// Direct comparison between native smooth compilation and degree-zero
/// compatibility lowering of the same requested shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoricalComparison {
    pub native: Compilation,
    pub categorical: Compilation,
    /// Difference between the two unquantized spline functions.
    pub spline_difference: NormBounds,
    /// Conservative difference after each basis's independent consensus
    /// weight quantization.
    pub consensus_sup_upper: BigRational,
    pub consensus_l1_upper: BigRational,
}

/// Compile a bounded shape against the exact open-clamped basis semantics.
pub fn compile(spec: &BasisSpec, shape: Shape) -> Result<Compilation, Error> {
    spec.validate().map_err(|_| Error::InvalidBasis)?;
    if spec.edge_policy != EdgePolicy::Clamp {
        return Err(Error::UnsupportedEdgePolicy);
    }
    validate_shape(shape)?;
    let basis = ExactBasis::new(spec)?;
    let height = rat_u64(shape.height());

    let exact = exact_construction(&basis, shape)?;
    let (status, construction, coefficients, sample_error) = if let Some(value) = exact {
        (
            SpanStatus::ExactInSpan,
            value.0,
            value.1,
            BigRational::zero(),
        )
    } else if matches!(shape, Shape::Gaussian { .. }) {
        let (coefficients, sample_error) = gaussian_coefficients(&basis, shape)?;
        (
            SpanStatus::CertifiedApproximation,
            Construction::GaussianIntervalSamples,
            coefficients,
            sample_error,
        )
    } else {
        (
            SpanStatus::CertifiedApproximation,
            Construction::SchoenbergGrevilleQuasiInterpolant,
            sample_shape_at_sites(&basis, shape)?,
            BigRational::zero(),
        )
    };

    validate_coefficients(&coefficients, &height, basis.outcomes)?;
    let max_coefficient = coefficients
        .iter()
        .cloned()
        .max()
        .ok_or(Error::InternalInvariant)?;
    let certificate = if status == SpanStatus::ExactInSpan {
        exact_certificate(spec, &height, sample_error)
    } else if matches!(shape, Shape::Gaussian { .. }) {
        gaussian_certificate(spec, &basis, shape, &height, sample_error)?
    } else {
        piecewise_certificate(spec, &basis, shape, &coefficients, &height, sample_error)?
    };

    Ok(Compilation {
        status,
        construction,
        coefficients,
        height,
        max_coefficient,
        certificate,
    })
}

/// Compile one shape natively and through a categorical compatibility basis,
/// then certify how far the resulting payout functions can differ.
///
/// Both bases must cover the exact same closed coordinate domain.  The second
/// basis must be degree zero; the function refuses rather than silently
/// comparing two unrelated smooth bases.
pub fn compare_categorical_lowering(
    native_spec: &BasisSpec,
    categorical_spec: &BasisSpec,
    shape: Shape,
) -> Result<CategoricalComparison, Error> {
    if categorical_spec.degree != 0 {
        return Err(Error::NotCategorical);
    }
    let native_basis = ExactBasis::new(native_spec)?;
    let categorical_basis = ExactBasis::new(categorical_spec)?;
    if native_basis.domain_low != categorical_basis.domain_low
        || native_basis.domain_high != categorical_basis.domain_high
    {
        return Err(Error::DomainMismatch);
    }

    let native = compile(native_spec, shape)?;
    let categorical = compile(categorical_spec, shape)?;
    let mut points = native_basis.distinct_breaks.clone();
    points.extend(categorical_basis.distinct_breaks.iter().cloned());
    points.sort();
    points.dedup();

    let lipschitz = native_basis.derivative_bound(&native.coefficients)?
        + categorical_basis.derivative_bound(&categorical.coefficients)?;
    let mut bounds = NormBounds {
        sup_lower: BigRational::zero(),
        sup_upper: BigRational::zero(),
        l1_lower: BigRational::zero(),
        l1_upper: BigRational::zero(),
    };
    for pair in points.windows(2) {
        certify_spline_difference(
            &native_basis,
            &native.coefficients,
            &categorical_basis,
            &categorical.coefficients,
            &pair[0],
            &pair[1],
            &lipschitz,
            CERTIFICATION_DEPTH,
            &mut bounds,
        )?;
    }
    for point in &points {
        let difference = (native_basis.spline(&native.coefficients, point)?
            - categorical_basis.spline(&categorical.coefficients, point)?)
        .abs();
        bounds.sup_lower = bounds.sup_lower.max(difference.clone());
        bounds.sup_upper = bounds.sup_upper.max(difference);
    }
    let height = rat_u64(shape.height());
    let domain = &native_basis.domain_high - &native_basis.domain_low;
    bounds.sup_upper = bounds.sup_upper.min(height.clone());
    bounds.l1_upper = bounds.l1_upper.min(&height * &domain);

    let quantization = &native.certificate.consensus_quantization_sup_upper
        + &categorical.certificate.consensus_quantization_sup_upper;
    let consensus_sup_upper = (&bounds.sup_upper + &quantization).min(height.clone());
    let consensus_l1_upper = &bounds.l1_upper + domain * quantization;
    Ok(CategoricalComparison {
        native,
        categorical,
        spline_difference: bounds,
        consensus_sup_upper,
        consensus_l1_upper,
    })
}

/// Exact value of the unquantized spline represented by `coefficients`.
pub fn spline_value(
    spec: &BasisSpec,
    coefficients: &[BigRational],
    x: BigRational,
) -> Result<BigRational, Error> {
    let basis = ExactBasis::new(spec)?;
    validate_coefficients(
        coefficients,
        &coefficients
            .iter()
            .cloned()
            .max()
            .unwrap_or_else(BigRational::zero),
        basis.outcomes,
    )?;
    basis.spline(coefficients, &x)
}

/// Evaluate the actual integer-weight consensus payout at one integer
/// coordinate, retaining the coefficient rationals exactly.
pub fn quantized_payout(
    spec: &BasisSpec,
    coefficients: &[BigRational],
    x: u128,
) -> Result<BigRational, Error> {
    if coefficients.len() != usize::from(spec.outcome_count) {
        return Err(Error::InternalInvariant);
    }
    let weights = spec.evaluate(x).map_err(|_| Error::InvalidBasis)?;
    let denominator = rat_u64(spec.denominator);
    let mut value = BigRational::zero();
    for (coefficient, weight) in coefficients.iter().zip(weights.weights.iter()) {
        value += coefficient * rat_u64(*weight) / &denominator;
    }
    Ok(value)
}

#[derive(Clone, Debug)]
struct ExactBasis {
    degree: usize,
    outcomes: usize,
    expanded: Vec<BigRational>,
    sites: Vec<BigRational>,
    domain_low: BigRational,
    domain_high: BigRational,
    distinct_breaks: Vec<BigRational>,
}

impl ExactBasis {
    fn new(spec: &BasisSpec) -> Result<Self, Error> {
        spec.validate().map_err(|_| Error::InvalidBasis)?;
        let degree = usize::from(spec.degree);
        let outcomes = usize::from(spec.outcome_count);
        if outcomes > MAX_OUTCOMES {
            return Err(Error::TooManyOutcomes);
        }
        if degree == 0 {
            let mut breaks = vec![rat_u128(0)];
            for knot in spec.knots.iter().take(usize::from(spec.knot_count)) {
                breaks.push(rat_u128(*knot));
            }
            breaks.push(rat_u128(spec.domain_max));
            let mut sites = Vec::with_capacity(outcomes);
            for pair in breaks.windows(2) {
                sites.push((&pair[0] + &pair[1]) / rat_i64(2));
            }
            return Ok(Self {
                degree,
                outcomes,
                expanded: Vec::new(),
                sites,
                domain_low: rat_u128(0),
                domain_high: rat_u128(spec.domain_max),
                distinct_breaks: breaks,
            });
        }

        let knots: Vec<BigRational> = spec
            .knots
            .iter()
            .take(usize::from(spec.knot_count))
            .map(|value| rat_u128(*value))
            .collect();
        let first = knots.first().cloned().ok_or(Error::InvalidBasis)?;
        let last = knots.last().cloned().ok_or(Error::InvalidBasis)?;
        let mut expanded = Vec::with_capacity(knots.len() + 2 * degree);
        for _ in 0..=degree {
            expanded.push(first.clone());
        }
        for knot in knots.iter().skip(1).take(knots.len().saturating_sub(2)) {
            expanded.push(knot.clone());
        }
        for _ in 0..=degree {
            expanded.push(last.clone());
        }
        if expanded.len() != outcomes + degree + 1 {
            return Err(Error::InternalInvariant);
        }
        let mut sites = Vec::with_capacity(outcomes);
        for index in 0..outcomes {
            let mut sum = BigRational::zero();
            for offset in 1..=degree {
                sum += &expanded[index + offset];
            }
            sites.push(sum / rat_usize(degree));
        }
        Ok(Self {
            degree,
            outcomes,
            expanded,
            sites,
            domain_low: first,
            domain_high: last,
            distinct_breaks: knots,
        })
    }

    fn weights(&self, x: &BigRational) -> Result<Vec<BigRational>, Error> {
        if self.degree == 0 {
            let handled = x.clamp(&self.domain_low, &self.domain_high);
            let mut weights = vec![BigRational::zero(); self.outcomes];
            let mut cell = 0_usize;
            while cell + 1 < self.outcomes && handled >= &self.distinct_breaks[cell + 1] {
                cell += 1;
            }
            weights[cell] = BigRational::one();
            return Ok(weights);
        }
        let handled = x.clamp(&self.domain_low, &self.domain_high);
        if handled == &self.domain_high {
            let mut weights = vec![BigRational::zero(); self.outcomes];
            weights[self.outcomes - 1] = BigRational::one();
            return Ok(weights);
        }

        let max_zero = self.expanded.len() - 1;
        let mut column = vec![BigRational::zero(); max_zero];
        for (index, value) in column.iter_mut().enumerate() {
            if self.expanded[index] <= *handled && *handled < self.expanded[index + 1] {
                *value = BigRational::one();
            }
        }
        for degree in 1..=self.degree {
            let mut next = vec![BigRational::zero(); max_zero - degree];
            for index in 0..next.len() {
                let left_den = &self.expanded[index + degree] - &self.expanded[index];
                if !left_den.is_zero() {
                    next[index] += ((handled - &self.expanded[index]) / left_den) * &column[index];
                }
                let right_den = &self.expanded[index + degree + 1] - &self.expanded[index + 1];
                if !right_den.is_zero() {
                    next[index] += ((&self.expanded[index + degree + 1] - handled) / right_den)
                        * &column[index + 1];
                }
            }
            column = next;
        }
        if column.len() != self.outcomes {
            return Err(Error::InternalInvariant);
        }
        let sum: BigRational = column.iter().cloned().sum();
        if sum != BigRational::one() || column.iter().any(|weight| weight.is_negative()) {
            return Err(Error::InternalInvariant);
        }
        Ok(column)
    }

    fn spline(&self, coefficients: &[BigRational], x: &BigRational) -> Result<BigRational, Error> {
        if coefficients.len() != self.outcomes {
            return Err(Error::InternalInvariant);
        }
        Ok(coefficients
            .iter()
            .zip(self.weights(x)?)
            .map(|(coefficient, weight)| coefficient * weight)
            .sum())
    }

    fn support_radius(&self) -> Result<BigRational, Error> {
        if self.degree == 0 {
            let mut radius = BigRational::zero();
            for (index, site) in self.sites.iter().enumerate() {
                radius = radius.max((site - &self.distinct_breaks[index]).abs());
                radius = radius.max((&self.distinct_breaks[index + 1] - site).abs());
            }
            return Ok(radius);
        }
        let mut radius = BigRational::zero();
        for (index, site) in self.sites.iter().enumerate() {
            radius = radius.max((site - &self.expanded[index]).abs());
            radius = radius.max((&self.expanded[index + self.degree + 1] - site).abs());
        }
        Ok(radius)
    }

    fn derivative_bound(&self, coefficients: &[BigRational]) -> Result<BigRational, Error> {
        if self.degree == 0 {
            return Ok(BigRational::zero());
        }
        let mut bound = BigRational::zero();
        for index in 0..self.outcomes - 1 {
            let denominator = &self.expanded[index + self.degree + 1] - &self.expanded[index + 1];
            if denominator.is_zero() {
                continue;
            }
            let derivative = rat_usize(self.degree)
                * (&coefficients[index + 1] - &coefficients[index])
                / denominator;
            bound = bound.max(derivative.abs());
        }
        Ok(bound)
    }
}

fn exact_construction(
    basis: &ExactBasis,
    shape: Shape,
) -> Result<Option<(Construction, Vec<BigRational>)>, Error> {
    if basis.degree == 0 && step_shape(shape) && degree_zero_shape_is_exact(basis, shape)? {
        return Ok(Some((
            Construction::DegreeZeroCells,
            sample_shape_at_sites(basis, shape)?,
        )));
    }
    if basis.degree == 1 && continuous_piecewise_linear(shape) && kinks_align(basis, shape) {
        return Ok(Some((
            Construction::DegreeOneInterpolation,
            sample_shape_at_sites(basis, shape)?,
        )));
    }
    if basis.degree >= 1 && continuous_piecewise_linear(shape) && no_interior_kink(basis, shape) {
        return Ok(Some((
            Construction::GrevilleAffineReproduction,
            sample_shape_at_sites(basis, shape)?,
        )));
    }
    if is_constant_on_domain(basis, shape)? {
        return Ok(Some((
            if basis.degree == 0 {
                Construction::DegreeZeroCells
            } else {
                Construction::GrevilleAffineReproduction
            },
            sample_shape_at_sites(basis, shape)?,
        )));
    }
    Ok(None)
}

fn sample_shape_at_sites(basis: &ExactBasis, shape: Shape) -> Result<Vec<BigRational>, Error> {
    basis
        .sites
        .iter()
        .map(|site| shape_value_exact(shape, site))
        .collect()
}

fn gaussian_coefficients(
    basis: &ExactBasis,
    shape: Shape,
) -> Result<(Vec<BigRational>, BigRational), Error> {
    let Shape::Gaussian {
        center,
        sigma,
        height,
    } = shape
    else {
        return Err(Error::InvalidShape);
    };
    let center = rat_u128(center);
    let sigma = rat_u128(sigma);
    let height = rat_u64(height);
    let mut coefficients = Vec::with_capacity(basis.outcomes);
    let mut sample_error = BigRational::zero();
    for site in &basis.sites {
        let distance = site - &center;
        let z = &distance * &distance / (rat_i64(2) * &sigma * &sigma);
        let enclosure = exp_neg_enclosure(z)?;
        let coefficient = (&enclosure.low + &enclosure.high) / rat_i64(2) * &height;
        let error = (&enclosure.high - &enclosure.low) / rat_i64(2) * &height;
        coefficients.push(coefficient);
        sample_error = sample_error.max(error);
    }
    Ok((coefficients, sample_error))
}

#[derive(Clone, Debug)]
struct Interval {
    low: BigRational,
    high: BigRational,
}

/// Alternating Taylor enclosure after exact power-of-two range reduction.
fn exp_neg_enclosure(z: BigRational) -> Result<Interval, Error> {
    if z.is_negative() {
        return Err(Error::InternalInvariant);
    }
    let original = z;
    let mut reduced = original.clone();
    let mut squarings = 0_u8;
    while reduced > rat_i64(1) / rat_i64(2) {
        if squarings == MAX_EXP_RANGE_SQUARINGS {
            // `e^(z/m) >= 1 + z/m`, hence
            // `e^-z <= (1 + z/m)^-m`.  This branch is intentionally one-sided
            // and computationally bounded: another exact squaring doubles
            // numerator and denominator bit lengths.
            let power = rat_u64(u64::from(EXP_FAR_TAIL_POWER));
            let base = BigRational::one() + original / power;
            return Ok(Interval {
                low: BigRational::zero(),
                high: BigRational::one() / rational_pow(base, EXP_FAR_TAIL_POWER),
            });
        }
        reduced /= rat_i64(2);
        squarings += 1;
    }
    let mut sum = BigRational::one();
    let mut term = BigRational::one();
    let mut lower = None;
    let mut upper = None;
    for order in 1..=18_i64 {
        term = term * &reduced / rat_i64(order);
        if order % 2 == 0 {
            sum += &term;
            if order == 18 {
                upper = Some(sum.clone());
            }
        } else {
            sum -= &term;
            if order == 17 {
                lower = Some(sum.clone());
            }
        }
    }
    let mut low = lower.ok_or(Error::InternalInvariant)?;
    let mut high = upper.ok_or(Error::InternalInvariant)?;
    for _ in 0..squarings {
        low = &low * &low;
        high = &high * &high;
    }
    low = low.max(BigRational::zero());
    high = high.min(BigRational::one());
    if low > high {
        return Err(Error::InternalInvariant);
    }
    Ok(Interval { low, high })
}

fn rational_pow(mut base: BigRational, mut exponent: u8) -> BigRational {
    let mut result = BigRational::one();
    while exponent > 0 {
        if exponent & 1 == 1 {
            result *= &base;
        }
        exponent >>= 1;
        if exponent > 0 {
            base = &base * &base;
        }
    }
    result
}

fn exact_certificate(
    spec: &BasisSpec,
    height: &BigRational,
    sample_error: BigRational,
) -> ErrorCertificate {
    finish_certificate(
        spec,
        height,
        BigRational::zero(),
        BigRational::zero(),
        BigRational::zero(),
        BigRational::zero(),
        sample_error,
    )
}

fn piecewise_certificate(
    spec: &BasisSpec,
    basis: &ExactBasis,
    shape: Shape,
    coefficients: &[BigRational],
    height: &BigRational,
    sample_error: BigRational,
) -> Result<ErrorCertificate, Error> {
    let mut points = basis.distinct_breaks.clone();
    for kink in shape.breakpoints() {
        let point = rat_u128(kink);
        if basis.domain_low < point && point < basis.domain_high {
            points.push(point);
        }
    }
    points.sort();
    points.dedup();

    let spline_lipschitz = basis.derivative_bound(coefficients)?;
    let shape_lipschitz = shape.lipschitz()?;
    let error_lipschitz = spline_lipschitz + shape_lipschitz;
    let mut sup_lower = BigRational::zero();
    let mut sup_upper = BigRational::zero();
    let mut l1_lower = BigRational::zero();
    let mut l1_upper = BigRational::zero();
    for pair in points.windows(2) {
        certify_interval(
            basis,
            shape,
            coefficients,
            &pair[0],
            &pair[1],
            &error_lipschitz,
            CERTIFICATION_DEPTH,
            &mut sup_lower,
            &mut sup_upper,
            &mut l1_lower,
            &mut l1_upper,
        )?;
    }
    for point in &points {
        let error = (shape_value_exact(shape, point)? - basis.spline(coefficients, point)?).abs();
        sup_lower = sup_lower.max(error.clone());
        sup_upper = sup_upper.max(error);
    }
    if basis.degree >= 1 && step_shape(shape) && has_interior_jump(basis, shape) {
        sup_lower = sup_lower.max(height / rat_i64(2));
    }
    sup_upper = sup_upper.min(height.clone());
    finish_certificate_result(
        spec,
        height,
        sup_lower,
        sup_upper,
        l1_lower,
        l1_upper,
        sample_error,
    )
}

#[allow(clippy::too_many_arguments)]
fn certify_interval(
    basis: &ExactBasis,
    shape: Shape,
    coefficients: &[BigRational],
    low: &BigRational,
    high: &BigRational,
    lipschitz: &BigRational,
    depth: u8,
    sup_lower: &mut BigRational,
    sup_upper: &mut BigRational,
    l1_lower: &mut BigRational,
    l1_upper: &mut BigRational,
) -> Result<(), Error> {
    if low >= high {
        return Ok(());
    }
    if depth > 0 {
        let middle = (low + high) / rat_i64(2);
        certify_interval(
            basis,
            shape,
            coefficients,
            low,
            &middle,
            lipschitz,
            depth - 1,
            sup_lower,
            sup_upper,
            l1_lower,
            l1_upper,
        )?;
        certify_interval(
            basis,
            shape,
            coefficients,
            &middle,
            high,
            lipschitz,
            depth - 1,
            sup_lower,
            sup_upper,
            l1_lower,
            l1_upper,
        )?;
        return Ok(());
    }
    let width = high - low;
    let middle = (low + high) / rat_i64(2);
    let error = (shape_value_exact(shape, &middle)? - basis.spline(coefficients, &middle)?).abs();
    let radius = lipschitz * &width / rat_i64(2);
    let upper = &error + &radius;
    let lower = (&error - &radius).max(BigRational::zero());
    *sup_lower = sup_lower.clone().max(error);
    *sup_upper = sup_upper.clone().max(upper.clone());
    *l1_lower += &width * lower;
    *l1_upper += width * upper;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn certify_spline_difference(
    left_basis: &ExactBasis,
    left_coefficients: &[BigRational],
    right_basis: &ExactBasis,
    right_coefficients: &[BigRational],
    low: &BigRational,
    high: &BigRational,
    lipschitz: &BigRational,
    depth: u8,
    bounds: &mut NormBounds,
) -> Result<(), Error> {
    if low >= high {
        return Ok(());
    }
    if depth > 0 {
        let middle = (low + high) / rat_i64(2);
        certify_spline_difference(
            left_basis,
            left_coefficients,
            right_basis,
            right_coefficients,
            low,
            &middle,
            lipschitz,
            depth - 1,
            bounds,
        )?;
        certify_spline_difference(
            left_basis,
            left_coefficients,
            right_basis,
            right_coefficients,
            &middle,
            high,
            lipschitz,
            depth - 1,
            bounds,
        )?;
        return Ok(());
    }
    let width = high - low;
    let middle = (low + high) / rat_i64(2);
    let difference = (left_basis.spline(left_coefficients, &middle)?
        - right_basis.spline(right_coefficients, &middle)?)
    .abs();
    let radius = lipschitz * &width / rat_i64(2);
    let upper = &difference + &radius;
    let lower = (&difference - &radius).max(BigRational::zero());
    bounds.sup_lower = bounds.sup_lower.clone().max(difference);
    bounds.sup_upper = bounds.sup_upper.clone().max(upper.clone());
    bounds.l1_lower += &width * lower;
    bounds.l1_upper += width * upper;
    Ok(())
}

fn gaussian_certificate(
    spec: &BasisSpec,
    basis: &ExactBasis,
    shape: Shape,
    height: &BigRational,
    sample_error: BigRational,
) -> Result<ErrorCertificate, Error> {
    let Shape::Gaussian { sigma, .. } = shape else {
        return Err(Error::InvalidShape);
    };
    // |f'| <= H/sigma follows from r*exp(-r^2/2) <= 1.  This deliberately
    // avoids importing an irrational 1/sqrt(e) constant into the certificate.
    let lipschitz = height / rat_u128(sigma);
    let rho = basis.support_radius()?;
    let sup_upper = (lipschitz * rho + &sample_error).min(height.clone());
    let domain = &basis.domain_high - &basis.domain_low;
    let l1_upper = &domain * &sup_upper;
    finish_certificate_result(
        spec,
        height,
        BigRational::zero(),
        sup_upper,
        BigRational::zero(),
        l1_upper,
        sample_error,
    )
}

fn finish_certificate_result(
    spec: &BasisSpec,
    height: &BigRational,
    spline_sup_lower: BigRational,
    spline_sup_upper: BigRational,
    spline_l1_lower: BigRational,
    spline_l1_upper: BigRational,
    sample_error: BigRational,
) -> Result<ErrorCertificate, Error> {
    if spline_sup_lower > spline_sup_upper || spline_l1_lower > spline_l1_upper {
        return Err(Error::InternalInvariant);
    }
    Ok(finish_certificate(
        spec,
        height,
        spline_sup_lower,
        spline_sup_upper,
        spline_l1_lower,
        spline_l1_upper,
        sample_error,
    ))
}

fn finish_certificate(
    spec: &BasisSpec,
    height: &BigRational,
    spline_sup_lower: BigRational,
    spline_sup_upper: BigRational,
    spline_l1_lower: BigRational,
    spline_l1_upper: BigRational,
    sample_error: BigRational,
) -> ErrorCertificate {
    let quantization = if spec.degree == 0 {
        BigRational::zero()
    } else {
        height * rat_u64(u64::from(spec.degree)) / rat_u64(spec.denominator)
    };
    let domain = if spec.degree == 0 {
        rat_u128(spec.domain_max)
    } else {
        rat_u128(spec.knots[usize::from(spec.knot_count) - 1] - spec.knots[0])
    };
    let consensus_sup = (&spline_sup_upper + &quantization).min(height.clone());
    let consensus_l1 = &spline_l1_upper + domain * &quantization;
    ErrorCertificate {
        spline_sup_lower,
        spline_sup_upper,
        spline_l1_lower,
        spline_l1_upper,
        consensus_quantization_sup_upper: quantization,
        consensus_sup_upper: consensus_sup,
        consensus_l1_upper: consensus_l1,
        coefficient_sample_sup_upper: sample_error,
        subdivision_depth: CERTIFICATION_DEPTH,
    }
}

fn validate_shape(shape: Shape) -> Result<(), Error> {
    if shape.height() == 0 {
        return Err(Error::InvalidShape);
    }
    match shape {
        Shape::HardRange { low, high, .. }
        | Shape::CappedCall { low, high, .. }
        | Shape::CappedPut { low, high, .. } => {
            if low >= high {
                return Err(Error::InvalidShape);
            }
        }
        Shape::Triangle {
            left, peak, right, ..
        } => {
            if !(left < peak && peak < right) {
                return Err(Error::InvalidShape);
            }
        }
        Shape::Gaussian { sigma: 0, .. } => return Err(Error::InvalidShape),
        _ => {}
    }
    Ok(())
}

fn validate_coefficients(
    coefficients: &[BigRational],
    height: &BigRational,
    outcomes: usize,
) -> Result<(), Error> {
    if coefficients.len() != outcomes
        || coefficients
            .iter()
            .any(|coefficient| coefficient.is_negative() || coefficient > height)
    {
        return Err(Error::InternalInvariant);
    }
    Ok(())
}

impl Shape {
    fn height(self) -> u64 {
        match self {
            Self::HardRange { height, .. }
            | Self::UpperTail { height, .. }
            | Self::LowerTail { height, .. }
            | Self::Triangle { height, .. }
            | Self::CappedCall { height, .. }
            | Self::CappedPut { height, .. }
            | Self::Gaussian { height, .. } => height,
        }
    }

    fn breakpoints(self) -> Vec<u128> {
        match self {
            Self::HardRange { low, high, .. } => vec![low, high],
            Self::UpperTail { strike, .. } | Self::LowerTail { strike, .. } => vec![strike],
            Self::Triangle {
                left, peak, right, ..
            } => vec![left, peak, right],
            Self::CappedCall { low, high, .. } | Self::CappedPut { low, high, .. } => {
                vec![low, high]
            }
            Self::Gaussian { .. } => Vec::new(),
        }
    }

    fn lipschitz(self) -> Result<BigRational, Error> {
        let height = rat_u64(self.height());
        match self {
            Self::HardRange { .. } | Self::UpperTail { .. } | Self::LowerTail { .. } => {
                Ok(BigRational::zero())
            }
            Self::Triangle {
                left, peak, right, ..
            } => Ok((height.clone() / rat_u128(peak - left)).max(height / rat_u128(right - peak))),
            Self::CappedCall { low, high, .. } | Self::CappedPut { low, high, .. } => {
                Ok(height / rat_u128(high - low))
            }
            Self::Gaussian { sigma, .. } => Ok(height / rat_u128(sigma)),
        }
    }
}

fn shape_value_exact(shape: Shape, x: &BigRational) -> Result<BigRational, Error> {
    let height = rat_u64(shape.height());
    Ok(match shape {
        Shape::HardRange { low, high, .. } => {
            if rat_u128(low) <= *x && *x < rat_u128(high) {
                height
            } else {
                BigRational::zero()
            }
        }
        Shape::UpperTail { strike, .. } => {
            if *x >= rat_u128(strike) {
                height
            } else {
                BigRational::zero()
            }
        }
        Shape::LowerTail { strike, .. } => {
            if *x < rat_u128(strike) {
                height
            } else {
                BigRational::zero()
            }
        }
        Shape::Triangle {
            left, peak, right, ..
        } => {
            let left = rat_u128(left);
            let peak = rat_u128(peak);
            let right = rat_u128(right);
            if *x <= left || *x >= right {
                BigRational::zero()
            } else if *x <= peak {
                height * (x - &left) / (peak - left)
            } else {
                height * (&right - x) / (right - peak)
            }
        }
        Shape::CappedCall { low, high, .. } => {
            let low = rat_u128(low);
            let high = rat_u128(high);
            if *x <= low {
                BigRational::zero()
            } else if *x >= high {
                height
            } else {
                height * (x - &low) / (high - low)
            }
        }
        Shape::CappedPut { low, high, .. } => {
            let low = rat_u128(low);
            let high = rat_u128(high);
            if *x <= low {
                height
            } else if *x >= high {
                BigRational::zero()
            } else {
                height * (&high - x) / (high - low)
            }
        }
        Shape::Gaussian { .. } => return Err(Error::InvalidShape),
    })
}

fn step_shape(shape: Shape) -> bool {
    matches!(
        shape,
        Shape::HardRange { .. } | Shape::UpperTail { .. } | Shape::LowerTail { .. }
    )
}

fn continuous_piecewise_linear(shape: Shape) -> bool {
    matches!(
        shape,
        Shape::Triangle { .. } | Shape::CappedCall { .. } | Shape::CappedPut { .. }
    )
}

fn degree_zero_shape_is_exact(basis: &ExactBasis, shape: Shape) -> Result<bool, Error> {
    if basis.degree != 0 || !step_shape(shape) {
        return Ok(false);
    }
    if shape.breakpoints().iter().any(|point| {
        let point = rat_u128(*point);
        basis.domain_low < point
            && point < basis.domain_high
            && !basis.distinct_breaks.iter().any(|knot| knot == &point)
    }) {
        return Ok(false);
    }

    let coefficients = sample_shape_at_sites(basis, shape)?;
    // Interior aligned boundaries inherit the target's right-hand convention,
    // exactly as degree-zero evaluation does.  The top is different: it is a
    // closed point owned by the last cell, so a target jump exactly there is
    // not representable by the categorical basis.
    for point in &basis.distinct_breaks {
        if shape_value_exact(shape, point)? != basis.spline(&coefficients, point)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn kinks_align(basis: &ExactBasis, shape: Shape) -> bool {
    shape.breakpoints().iter().all(|point| {
        let point = rat_u128(*point);
        point <= basis.domain_low
            || point >= basis.domain_high
            || basis.distinct_breaks.iter().any(|knot| knot == &point)
    })
}

fn no_interior_kink(basis: &ExactBasis, shape: Shape) -> bool {
    !shape.breakpoints().iter().any(|point| {
        let point = rat_u128(*point);
        basis.domain_low < point && point < basis.domain_high
    })
}

fn has_interior_jump(basis: &ExactBasis, shape: Shape) -> bool {
    step_shape(shape)
        && shape.breakpoints().iter().any(|point| {
            let point = rat_u128(*point);
            basis.domain_low < point && point < basis.domain_high
        })
}

fn is_constant_on_domain(basis: &ExactBasis, shape: Shape) -> Result<bool, Error> {
    if matches!(shape, Shape::Gaussian { .. }) {
        return Ok(false);
    }
    let low = shape_value_exact(shape, &basis.domain_low)?;
    let high = shape_value_exact(shape, &basis.domain_high)?;
    let middle = shape_value_exact(
        shape,
        &((&basis.domain_low + &basis.domain_high) / rat_i64(2)),
    )?;
    Ok(low == high && high == middle && no_interior_kink(basis, shape))
}

fn rat_i64(value: i64) -> BigRational {
    BigRational::from_integer(BigInt::from(value))
}

fn rat_u64(value: u64) -> BigRational {
    BigRational::from_integer(BigInt::from(value))
}

fn rat_usize(value: usize) -> BigRational {
    BigRational::from_integer(BigInt::from(value))
}

fn rat_u128(value: u128) -> BigRational {
    BigRational::from_integer(BigInt::from(value))
}

/// Render a rational for stable reports and golden assertions.
pub fn rational_string(value: &BigRational) -> String {
    if value.denom().is_one() {
        value.numer().to_string()
    } else {
        format!("{}/{}", value.numer(), value.denom())
    }
}

/// Conservative decimal rendering for human inspection only.  Never use this
/// function to create consensus bytes or compare certificates.
pub fn rational_to_f64_for_display(value: &BigRational) -> Result<f64, Error> {
    value.to_f64().ok_or(Error::ArithmeticConversion)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_bspline::{MAX_KNOTS, UNIFORM_SPACING_NONE};

    fn smooth_spec(degree: u8, knots_in: &[u128], denominator: u64) -> BasisSpec {
        let mut knots = [0_u128; MAX_KNOTS];
        knots[..knots_in.len()].copy_from_slice(knots_in);
        BasisSpec {
            outcome_count: knots_in.len() as u8 - 1 + degree,
            degree,
            knot_count: knots_in.len() as u8,
            uniform_log2_spacing: if degree >= 2 { 2 } else { UNIFORM_SPACING_NONE },
            denominator,
            domain_max: *knots_in.last().unwrap(),
            edge_policy: EdgePolicy::Clamp,
            knots,
        }
    }

    fn categorical_spec(boundaries: &[u128], domain_max: u128, denominator: u64) -> BasisSpec {
        let mut knots = [0_u128; MAX_KNOTS];
        knots[..boundaries.len()].copy_from_slice(boundaries);
        BasisSpec {
            outcome_count: boundaries.len() as u8 + 1,
            degree: 0,
            knot_count: boundaries.len() as u8,
            uniform_log2_spacing: UNIFORM_SPACING_NONE,
            denominator,
            domain_max,
            edge_policy: EdgePolicy::Clamp,
            knots,
        }
    }

    #[test]
    fn hard_ranges_and_tails_are_exact_only_in_aligned_categorical_cells() {
        let categorical = categorical_spec(&[4, 8, 12], 16, 64);
        let range = compile(
            &categorical,
            Shape::HardRange {
                low: 4,
                high: 12,
                height: 10,
            },
        )
        .unwrap();
        assert_eq!(range.status, SpanStatus::ExactInSpan);
        assert_eq!(
            range.coefficients,
            vec![rat_i64(0), rat_i64(10), rat_i64(10), rat_i64(0)]
        );

        let quadratic = smooth_spec(2, &[0, 4, 8, 12, 16], 256);
        let tail = compile(
            &quadratic,
            Shape::UpperTail {
                strike: 8,
                height: 10,
            },
        )
        .unwrap();
        assert_eq!(tail.status, SpanStatus::CertifiedApproximation);
        assert!(tail.certificate.spline_sup_lower >= rat_i64(5));
        assert!(tail.certificate.spline_sup_upper <= rat_i64(10));
    }

    #[test]
    fn closed_top_discontinuities_are_never_mislabeled_exact() {
        let categorical = categorical_spec(&[4, 8, 12], 16, 64);
        for shape in [
            Shape::HardRange {
                low: 4,
                high: 16,
                height: 10,
            },
            Shape::UpperTail {
                strike: 16,
                height: 10,
            },
            Shape::LowerTail {
                strike: 16,
                height: 10,
            },
        ] {
            let compiled = compile(&categorical, shape).unwrap();
            assert_eq!(compiled.status, SpanStatus::CertifiedApproximation);
            assert_eq!(compiled.certificate.spline_sup_lower, rat_i64(10));
            assert_eq!(compiled.certificate.spline_sup_upper, rat_i64(10));
        }

        // A jump outside the domain has no semantic effect and can still be
        // represented exactly.
        let outside = compile(
            &categorical,
            Shape::UpperTail {
                strike: 17,
                height: 10,
            },
        )
        .unwrap();
        assert_eq!(outside.status, SpanStatus::ExactInSpan);
        assert!(outside.coefficients.iter().all(BigRational::is_zero));
    }

    #[test]
    fn triangles_and_capped_linear_shapes_are_exact_on_degree_one_knots() {
        let basis = smooth_spec(1, &[0, 4, 8, 12], 256);
        let triangle = compile(
            &basis,
            Shape::Triangle {
                left: 0,
                peak: 4,
                right: 12,
                height: 12,
            },
        )
        .unwrap();
        assert_eq!(triangle.status, SpanStatus::ExactInSpan);
        assert_eq!(
            triangle.coefficients,
            vec![rat_i64(0), rat_i64(12), rat_i64(6), rat_i64(0)]
        );
        assert!(triangle.certificate.spline_sup_upper.is_zero());

        for shape in [
            Shape::CappedCall {
                low: 4,
                high: 8,
                height: 20,
            },
            Shape::CappedPut {
                low: 4,
                high: 8,
                height: 20,
            },
        ] {
            assert_eq!(
                compile(&basis, shape).unwrap().status,
                SpanStatus::ExactInSpan
            );
        }
    }

    #[test]
    fn degree_two_and_three_samples_are_certified_not_called_interpolation() {
        for degree in [2, 3] {
            let basis = smooth_spec(degree, &[0, 4, 8, 12, 16], 1024);
            let compiled = compile(
                &basis,
                Shape::Triangle {
                    left: 0,
                    peak: 8,
                    right: 16,
                    height: 16,
                },
            )
            .unwrap();
            assert_eq!(compiled.status, SpanStatus::CertifiedApproximation);
            assert_eq!(
                compiled.construction,
                Construction::SchoenbergGrevilleQuasiInterpolant
            );
            assert!(compiled.certificate.spline_sup_upper > BigRational::zero());
            assert!(compiled.certificate.spline_l1_upper > BigRational::zero());
        }
    }

    #[test]
    fn affine_payoffs_use_exact_greville_reproduction_in_smooth_degrees() {
        for degree in [2, 3] {
            let basis = smooth_spec(degree, &[0, 4, 8, 12, 16], 65_536);
            let shape = Shape::CappedCall {
                low: 0,
                high: 16,
                height: 32,
            };
            let compiled = compile(&basis, shape).unwrap();
            assert_eq!(compiled.status, SpanStatus::ExactInSpan);
            assert_eq!(
                compiled.construction,
                Construction::GrevilleAffineReproduction
            );
            assert!(compiled.certificate.spline_sup_upper.is_zero());

            let exact = ExactBasis::new(&basis).unwrap();
            for numerator in 0..=128_i64 {
                let x = rat_i64(numerator) / rat_i64(8);
                assert_eq!(
                    exact.spline(&compiled.coefficients, &x).unwrap(),
                    shape_value_exact(shape, &x).unwrap()
                );
            }
        }
    }

    #[test]
    fn convex_hull_prevents_overshoot_even_on_clamped_edges_and_knots() {
        for degree in [1, 2, 3] {
            let basis = smooth_spec(degree, &[0, 4, 8, 12, 16], 4096);
            let compiled = compile(
                &basis,
                Shape::CappedCall {
                    low: 4,
                    high: 12,
                    height: 100,
                },
            )
            .unwrap();
            let exact = ExactBasis::new(&basis).unwrap();
            for numerator in 0..=64_i64 {
                let x = rat_i64(numerator) / rat_i64(4);
                let value = exact.spline(&compiled.coefficients, &x).unwrap();
                assert!(value >= BigRational::zero());
                assert!(value <= rat_i64(100));
            }
            for knot in [0, 4, 8, 12, 16] {
                let value = exact
                    .spline(&compiled.coefficients, &rat_i64(knot))
                    .unwrap();
                assert!(value >= BigRational::zero() && value <= rat_i64(100));
            }
        }
    }

    #[test]
    fn narrow_gaussian_has_rational_enclosures_and_bounded_coefficients() {
        let basis = smooth_spec(3, &[0, 4, 8, 12, 16], 65_536);
        let compiled = compile(
            &basis,
            Shape::Gaussian {
                center: 8,
                sigma: 1,
                height: 1_000,
            },
        )
        .unwrap();
        assert_eq!(compiled.status, SpanStatus::CertifiedApproximation);
        assert_eq!(compiled.construction, Construction::GaussianIntervalSamples);
        assert!(compiled
            .coefficients
            .iter()
            .all(|value| { value >= &BigRational::zero() && value <= &rat_i64(1_000) }));
        assert!(compiled.certificate.coefficient_sample_sup_upper > BigRational::zero());
        assert!(compiled.certificate.consensus_sup_upper <= rat_i64(1_000));
    }

    #[test]
    fn gaussian_edges_and_extreme_distance_remain_bounded_and_total() {
        let basis = smooth_spec(3, &[0, 4, 8, 12, 16], 65_536);
        for center in [0, 16] {
            let compiled = compile(
                &basis,
                Shape::Gaussian {
                    center,
                    sigma: 1,
                    height: 1_000,
                },
            )
            .unwrap();
            assert!(compiled
                .coefficients
                .iter()
                .all(|value| value >= &BigRational::zero() && value <= &rat_i64(1_000)));
        }

        let far = compile(
            &basis,
            Shape::Gaussian {
                center: u128::MAX,
                sigma: 1,
                height: 1_000,
            },
        )
        .unwrap();
        assert!(far
            .coefficients
            .iter()
            .all(|value| value >= &BigRational::zero() && value < &rat_i64(1)));
        assert!(far.certificate.coefficient_sample_sup_upper < rat_i64(1));
    }

    #[test]
    fn categorical_lowering_does_not_relabel_native_triangle() {
        let native = smooth_spec(1, &[0, 4, 8], 256);
        let categorical = categorical_spec(&[4], 8, 256);
        let shape = Shape::Triangle {
            left: 0,
            peak: 4,
            right: 8,
            height: 8,
        };
        let native_compilation = compile(&native, shape).unwrap();
        let lowered = compile(&categorical, shape).unwrap();
        assert_eq!(native_compilation.status, SpanStatus::ExactInSpan);
        assert_eq!(lowered.status, SpanStatus::CertifiedApproximation);
        assert_eq!(lowered.coefficients, vec![rat_i64(4), rat_i64(4)]);
        assert!(lowered.certificate.spline_sup_upper >= rat_i64(4));
    }

    #[test]
    fn categorical_comparison_certifies_representation_difference() {
        let native = smooth_spec(1, &[0, 4, 8], 256);
        let categorical = categorical_spec(&[2, 4, 6], 8, 256);
        let shape = Shape::Triangle {
            left: 0,
            peak: 4,
            right: 8,
            height: 8,
        };
        let comparison = compare_categorical_lowering(&native, &categorical, shape).unwrap();
        assert_eq!(comparison.native.status, SpanStatus::ExactInSpan);
        assert_eq!(
            comparison.categorical.status,
            SpanStatus::CertifiedApproximation
        );
        assert!(comparison.spline_difference.sup_lower > BigRational::zero());
        assert!(comparison.spline_difference.sup_upper <= rat_i64(8));
        assert!(comparison.spline_difference.l1_upper <= rat_i64(64));

        let native_exact = ExactBasis::new(&native).unwrap();
        let categorical_exact = ExactBasis::new(&categorical).unwrap();
        for numerator in 0..=128_i64 {
            let x = rat_i64(numerator) / rat_i64(16);
            let difference = (native_exact
                .spline(&comparison.native.coefficients, &x)
                .unwrap()
                - categorical_exact
                    .spline(&comparison.categorical.coefficients, &x)
                    .unwrap())
            .abs();
            assert!(difference <= comparison.spline_difference.sup_upper);
        }
    }

    #[test]
    fn categorical_comparison_refuses_wrong_basis_or_domain() {
        let native = smooth_spec(1, &[0, 4, 8], 256);
        let smooth_again = smooth_spec(1, &[0, 4, 8], 256);
        let short_categorical = categorical_spec(&[2, 4], 6, 256);
        let shape = Shape::CappedPut {
            low: 2,
            high: 6,
            height: 8,
        };
        assert_eq!(
            compare_categorical_lowering(&native, &smooth_again, shape),
            Err(Error::NotCategorical)
        );
        assert_eq!(
            compare_categorical_lowering(&native, &short_categorical, shape),
            Err(Error::DomainMismatch)
        );
    }

    #[test]
    fn consensus_quantization_is_separate_from_spline_error() {
        let basis = smooth_spec(1, &[0, 4, 8], 7);
        let compiled = compile(
            &basis,
            Shape::CappedCall {
                low: 0,
                high: 8,
                height: 8,
            },
        )
        .unwrap();
        assert_eq!(compiled.status, SpanStatus::ExactInSpan);
        assert!(compiled.certificate.spline_sup_upper.is_zero());
        assert_eq!(
            compiled.certificate.consensus_quantization_sup_upper,
            rat_i64(8) / rat_i64(7)
        );
        let actual = quantized_payout(&basis, &compiled.coefficients, 2).unwrap();
        assert!((actual - rat_i64(2)).abs() <= rat_i64(8) / rat_i64(7));
    }

    #[test]
    fn consensus_quantization_bound_covers_every_degree_and_integer_point() {
        for degree in [1, 2, 3] {
            let basis = smooth_spec(degree, &[0, 4, 8, 12, 16], 7);
            let compiled = compile(
                &basis,
                Shape::Triangle {
                    left: 1,
                    peak: 7,
                    right: 15,
                    height: 19,
                },
            )
            .unwrap();
            for x in 0..=16 {
                let unquantized =
                    spline_value(&basis, &compiled.coefficients, rat_u128(x)).unwrap();
                let consensus = quantized_payout(&basis, &compiled.coefficients, x).unwrap();
                assert!(
                    (unquantized - consensus).abs()
                        <= compiled.certificate.consensus_quantization_sup_upper,
                    "degree={degree} x={x}",
                );
            }
        }
    }

    #[test]
    fn piecewise_certificates_cover_dense_exact_adversarial_points() {
        let categorical = categorical_spec(&[4, 8, 12], 16, 31);
        let smooth = [
            smooth_spec(1, &[0, 4, 8, 12, 16], 31),
            smooth_spec(2, &[0, 4, 8, 12, 16], 31),
            smooth_spec(3, &[0, 4, 8, 12, 16], 31),
        ];
        let shapes = [
            Shape::HardRange {
                low: 3,
                high: 13,
                height: 17,
            },
            Shape::UpperTail {
                strike: 7,
                height: 17,
            },
            Shape::LowerTail {
                strike: 9,
                height: 17,
            },
            Shape::Triangle {
                left: 1,
                peak: 7,
                right: 15,
                height: 17,
            },
            Shape::CappedCall {
                low: 3,
                high: 13,
                height: 17,
            },
            Shape::CappedPut {
                low: 3,
                high: 13,
                height: 17,
            },
        ];

        for basis_spec in core::iter::once(&categorical).chain(smooth.iter()) {
            let basis = ExactBasis::new(basis_spec).unwrap();
            for shape in shapes {
                let compiled = compile(basis_spec, shape).unwrap();
                assert!(
                    compiled.certificate.spline_sup_lower <= compiled.certificate.spline_sup_upper
                );
                assert!(
                    compiled.certificate.spline_l1_lower <= compiled.certificate.spline_l1_upper
                );
                for numerator in 0..=512_i64 {
                    let x = rat_i64(numerator) / rat_i64(32);
                    let error = (shape_value_exact(shape, &x).unwrap()
                        - basis.spline(&compiled.coefficients, &x).unwrap())
                    .abs();
                    assert!(
                        error <= compiled.certificate.spline_sup_upper,
                        "degree={} shape={shape:?} x={} error={} upper={}",
                        basis_spec.degree,
                        rational_string(&x),
                        rational_string(&error),
                        rational_string(&compiled.certificate.spline_sup_upper),
                    );
                }
            }
        }
    }
}
