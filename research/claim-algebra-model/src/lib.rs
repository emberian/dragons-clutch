#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Bounded payoff compilation over a finite partition-of-unity basis.
//!
//! This is an offline executable model, not a consensus codec or deployment
//! artifact. See the crate README for its exact boundary and proof claims.

/// Maximum basis size admitted by the current prototype family.
pub const MAX_OUTCOMES: usize = 16;

/// Largest coordinate admitted by the certified Gaussian compiler.
///
/// This bound keeps every product in the interval evaluator below `u128::MAX`
/// with the frozen scale and term count. Exact sample tables have no coordinate
/// domain and are unaffected by this model-only limit.
pub const MAX_GAUSSIAN_COORDINATE: u64 = 1_000_000_000;

const EXP_SCALE: u128 = 1_u128 << 40;
const EXP_TERMS: u32 = 32;
const EXP_CUTOFF: u128 = 8;
const EXP_CUTOFF_ERROR_DENOMINATOR: u128 = 2_048;

/// A refusal from the bounded compiler or payoff checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Active basis length is outside `1..=MAX_OUTCOMES`.
    InvalidOutcomeCount,
    /// An index or half-open range is outside the active basis.
    InvalidRange,
    /// A required quantity, scale, width, or denominator is zero.
    ZeroParameter,
    /// Knot coordinates are not strictly increasing.
    InvalidKnotGrid,
    /// A parameter exceeds the certified algorithm's frozen arithmetic domain.
    ParameterOutOfRange,
    /// Inactive fixed-array entries are not canonical zero padding.
    NonCanonicalPadding,
    /// Settlement weights are negative-by-representation impossible but do not
    /// sum exactly to their positive denominator.
    InvalidWeights,
    /// A checked integer operation exceeded its frozen width.
    ArithmeticOverflow,
    /// A requested redemption has a fractional collateral-atom remainder.
    RemainderRequired,
}

/// Result alias for the model.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact ordered grid used by coordinate-aware payoff constructors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnotGrid {
    /// Number of active entries in `knots`.
    pub active_len: u8,
    /// Strictly increasing active knot coordinates; inactive entries are zero.
    pub knots: [u64; MAX_OUTCOMES],
}

impl KnotGrid {
    /// Validate bounds, strict order, and canonical padding.
    pub fn validate(&self) -> Result<()> {
        let active = usize::from(self.active_len);
        if active == 0 || active > MAX_OUTCOMES {
            return Err(Error::InvalidOutcomeCount);
        }
        let mut i = 0_usize;
        while i < active {
            if i > 0 && self.knots[i - 1] >= self.knots[i] {
                return Err(Error::InvalidKnotGrid);
            }
            i += 1;
        }
        while i < MAX_OUTCOMES {
            if self.knots[i] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            i += 1;
        }
        Ok(())
    }
}

/// Frozen, non-Turing-complete payoff constructors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayoffSpecV1 {
    /// The same coefficient at every basis anchor.
    Constant {
        /// Number of active basis claims.
        active_len: u8,
        /// Exact coefficient in primitive Egg atoms.
        amount: u64,
    },
    /// One categorical basis claim.
    Categorical {
        /// Number of active basis claims.
        active_len: u8,
        /// Selected basis index.
        outcome: u8,
        /// Exact coefficient in primitive Egg atoms.
        amount: u64,
    },
    /// A contiguous half-open range of basis claims.
    HardRange {
        /// Number of active basis claims.
        active_len: u8,
        /// First included basis index.
        first: u8,
        /// First excluded basis index.
        end: u8,
        /// Exact coefficient applied inside the range.
        amount: u64,
    },
    /// A triangular curve sampled at the frozen grid.
    Triangle {
        /// Frozen basis anchors.
        grid: KnotGrid,
        /// Coordinate at which the target becomes nonzero.
        left: u64,
        /// Coordinate of full height.
        peak: u64,
        /// Coordinate at which the target returns to zero.
        right: u64,
        /// Exact height in primitive Egg atoms.
        height: u64,
    },
    /// A line between two coordinates, clamped to its endpoint amounts.
    CappedLinear {
        /// Frozen basis anchors.
        grid: KnotGrid,
        /// Coordinate carrying `start_amount`.
        start: u64,
        /// Coordinate carrying `end_amount`.
        end: u64,
        /// Amount at and below `start`.
        start_amount: u64,
        /// Amount at and above `end`.
        end_amount: u64,
    },
    /// An arbitrary exact bounded vector at the admitted finite basis.
    ///
    /// This is the semantic representation for externally compiled
    /// piecewise-polynomial, tabulated kernel, and other curve families. The
    /// exact vector is authoritative; an analytic label is not.
    ExactSamples {
        /// Number of active coefficients.
        active_len: u8,
        /// Exact active coefficients followed by canonical zero padding.
        coefficients: [u64; MAX_OUTCOMES],
    },
    /// A Gaussian curve compiled with the model's fixed interval algorithm.
    GaussianApprox {
        /// Frozen basis anchors.
        grid: KnotGrid,
        /// Center coordinate.
        center: u64,
        /// Positive standard-deviation coordinate.
        sigma: u64,
        /// Exact peak height in primitive Egg atoms.
        height: u64,
    },
}

/// Constructor identity carried by a compiled artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PayoffKindV1 {
    /// Constant constructor.
    Constant = 0,
    /// Categorical constructor.
    Categorical = 1,
    /// Hard-range constructor.
    HardRange = 2,
    /// Triangle constructor.
    Triangle = 3,
    /// Capped-linear constructor.
    CappedLinear = 4,
    /// Exact sampled-table constructor.
    ExactSamples = 5,
    /// Certified Gaussian approximation constructor.
    GaussianApprox = 6,
}

/// Conservative error bound for the source curve, in coefficient atoms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApproximationBound {
    /// Maximum error of a compiled knot coefficient against the analytic
    /// Gaussian value. Zero for exact constructors.
    pub knot_error_atoms: u128,
    /// Maximum piecewise-linear interpolation error between adjacent knots
    /// from the analytic Gaussian's curvature. Zero for exact constructors.
    pub linear_interpolation_error_atoms: u128,
    /// Maximum nearest-anchor, one-hot cell error from the analytic Gaussian's
    /// Lipschitz bound. Zero for exact constructors.
    pub one_hot_step_error_atoms: u128,
    /// Knot plus piecewise-linear interpolation error.
    pub total_linear_error_atoms: u128,
    /// Knot plus nearest-anchor one-hot cell error.
    pub total_one_hot_error_atoms: u128,
}

impl ApproximationBound {
    const ZERO: Self = Self {
        knot_error_atoms: 0,
        linear_interpolation_error_atoms: 0,
        one_hot_step_error_atoms: 0,
        total_linear_error_atoms: 0,
        total_one_hot_error_atoms: 0,
    };
}

/// Exact compiler output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledClaimV1 {
    /// Constructor that produced the vector.
    kind: PayoffKindV1,
    /// Number of active coefficients.
    active_len: u8,
    /// Primitive Egg atoms escrowed by one basket lot.
    coefficients: [u64; MAX_OUTCOMES],
    /// Exact full-simplex worst-case payout of one basket lot.
    maximum_payout_atoms: u64,
    /// Conservative analytic approximation certificate.
    approximation: ApproximationBound,
}

impl CompiledClaimV1 {
    /// Constructor identity.
    pub const fn kind(&self) -> PayoffKindV1 {
        self.kind
    }

    /// Number of active coefficients.
    pub const fn active_len(&self) -> u8 {
        self.active_len
    }

    /// Exact fixed-capacity coefficient vector.
    pub const fn coefficients(&self) -> &[u64; MAX_OUTCOMES] {
        &self.coefficients
    }

    /// Full-simplex worst-case payout of one basket lot.
    pub const fn maximum_payout_atoms(&self) -> u64 {
        self.maximum_payout_atoms
    }

    /// Conservative source-curve approximation bound.
    pub const fn approximation(&self) -> ApproximationBound {
        self.approximation
    }

    /// Check active shape and canonical padding, then recompute the maximum.
    pub fn validate(&self) -> Result<()> {
        validate_coefficients(self.active_len, &self.coefficients)?;
        if maximum(self.active_len, &self.coefficients)? != self.maximum_payout_atoms {
            return Err(Error::InvalidRange);
        }
        let linear_total = self
            .approximation
            .knot_error_atoms
            .checked_add(self.approximation.linear_interpolation_error_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        let one_hot_total = self
            .approximation
            .knot_error_atoms
            .checked_add(self.approximation.one_hot_step_error_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        if linear_total != self.approximation.total_linear_error_atoms
            || one_hot_total != self.approximation.total_one_hot_error_atoms
        {
            return Err(Error::InvalidRange);
        }
        if self.kind != PayoffKindV1::GaussianApprox
            && self.approximation != ApproximationBound::ZERO
        {
            return Err(Error::InvalidRange);
        }
        Ok(())
    }

    /// Conservative collateral atoms needed for `lots` identical baskets.
    pub fn worst_case_liability(&self, lots: u64) -> Result<u128> {
        self.validate()?;
        u128::from(self.maximum_payout_atoms)
            .checked_mul(u128::from(lots))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Ceiling-rounded payout under a validated simplex weight vector.
    ///
    /// This is a liability calculation, not a redemption rule. Use
    /// [`Self::exact_payout`] to enforce atom-exact redemption.
    pub fn ceiling_payout(&self, lots: u64, weights: &WeightVector) -> Result<u128> {
        let numerator = self.payout_numerator(lots, weights)?;
        div_ceil(numerator, u128::from(weights.denominator))
    }

    /// Exact payout, refusing any fractional collateral-atom remainder.
    pub fn exact_payout(&self, lots: u64, weights: &WeightVector) -> Result<u128> {
        let numerator = self.payout_numerator(lots, weights)?;
        let denominator = u128::from(weights.denominator);
        if !numerator.is_multiple_of(denominator) {
            return Err(Error::RemainderRequired);
        }
        Ok(numerator / denominator)
    }

    /// Exact terminal payout when primitive Eggs remain one-hot.
    ///
    /// Resolution selects one basis cell and each Egg atom in that cell pays
    /// one collateral atom. Therefore every integer portfolio redeems exactly
    /// with no denominator, remainder state, or minimum lot.
    pub fn one_hot_payout(&self, lots: u64, realized_outcome: u8) -> Result<u128> {
        self.validate()?;
        if realized_outcome >= self.active_len {
            return Err(Error::InvalidRange);
        }
        u128::from(self.coefficients[usize::from(realized_outcome)])
            .checked_mul(u128::from(lots))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Least positive basket count whose payout is integral for every integer
    /// simplex weight vector with the supplied denominator.
    pub fn universal_exact_lot(&self, denominator: u64) -> Result<u64> {
        self.validate()?;
        if denominator == 0 {
            return Err(Error::ZeroParameter);
        }
        let active = usize::from(self.active_len);
        let base = self.coefficients[0];
        let mut lot = 1_u64;
        let mut i = 1_usize;
        while i < active {
            let difference = self.coefficients[i].abs_diff(base);
            let factor = denominator / gcd(denominator, difference);
            lot = lcm(lot, factor)?;
            i += 1;
        }
        Ok(lot)
    }

    fn payout_numerator(&self, lots: u64, weights: &WeightVector) -> Result<u128> {
        self.validate()?;
        weights.validate()?;
        if self.active_len != weights.active_len {
            return Err(Error::InvalidOutcomeCount);
        }
        let active = usize::from(self.active_len);
        let mut sum = 0_u128;
        let mut i = 0_usize;
        while i < active {
            let term = u128::from(self.coefficients[i])
                .checked_mul(u128::from(weights.weights[i]))
                .and_then(|value| value.checked_mul(u128::from(lots)))
                .ok_or(Error::ArithmeticOverflow)?;
            sum = sum.checked_add(term).ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        Ok(sum)
    }
}

/// Integer partition-of-unity settlement weights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeightVector {
    /// Number of active weights.
    pub active_len: u8,
    /// Positive common denominator.
    pub denominator: u64,
    /// Nonnegative weights summing exactly to `denominator`, then zero padding.
    pub weights: [u64; MAX_OUTCOMES],
}

impl WeightVector {
    /// Validate shape, padding, per-weight bounds, and exact partition unity.
    pub fn validate(&self) -> Result<()> {
        let active = usize::from(self.active_len);
        if active == 0 || active > MAX_OUTCOMES {
            return Err(Error::InvalidOutcomeCount);
        }
        if self.denominator == 0 {
            return Err(Error::ZeroParameter);
        }
        let mut sum = 0_u128;
        let mut i = 0_usize;
        while i < active {
            if self.weights[i] > self.denominator {
                return Err(Error::InvalidWeights);
            }
            sum = sum
                .checked_add(u128::from(self.weights[i]))
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        while i < MAX_OUTCOMES {
            if self.weights[i] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            i += 1;
        }
        if sum != u128::from(self.denominator) {
            return Err(Error::InvalidWeights);
        }
        Ok(())
    }
}

/// Compile one frozen payoff specification.
pub fn compile(spec: PayoffSpecV1) -> Result<CompiledClaimV1> {
    let mut coefficients = [0_u64; MAX_OUTCOMES];
    let (kind, active_len, approximation) = match spec {
        PayoffSpecV1::Constant { active_len, amount } => {
            let active = checked_active(active_len)?;
            let mut i = 0_usize;
            while i < active {
                coefficients[i] = amount;
                i += 1;
            }
            (PayoffKindV1::Constant, active_len, ApproximationBound::ZERO)
        }
        PayoffSpecV1::Categorical {
            active_len,
            outcome,
            amount,
        } => {
            let active = checked_active(active_len)?;
            if usize::from(outcome) >= active {
                return Err(Error::InvalidRange);
            }
            coefficients[usize::from(outcome)] = amount;
            (
                PayoffKindV1::Categorical,
                active_len,
                ApproximationBound::ZERO,
            )
        }
        PayoffSpecV1::HardRange {
            active_len,
            first,
            end,
            amount,
        } => {
            let active = checked_active(active_len)?;
            if first >= end || usize::from(end) > active {
                return Err(Error::InvalidRange);
            }
            let mut i = usize::from(first);
            while i < usize::from(end) {
                coefficients[i] = amount;
                i += 1;
            }
            (
                PayoffKindV1::HardRange,
                active_len,
                ApproximationBound::ZERO,
            )
        }
        PayoffSpecV1::Triangle {
            grid,
            left,
            peak,
            right,
            height,
        } => {
            grid.validate()?;
            if left >= peak || peak >= right {
                return Err(Error::InvalidRange);
            }
            let active = usize::from(grid.active_len);
            let mut i = 0_usize;
            while i < active {
                let x = grid.knots[i];
                coefficients[i] = if x <= left || x >= right {
                    0
                } else if x == peak {
                    height
                } else if x < peak {
                    mul_div_floor(
                        u128::from(height),
                        u128::from(x - left),
                        u128::from(peak - left),
                    )?
                    .try_into()
                    .map_err(|_| Error::ArithmeticOverflow)?
                } else {
                    mul_div_floor(
                        u128::from(height),
                        u128::from(right - x),
                        u128::from(right - peak),
                    )?
                    .try_into()
                    .map_err(|_| Error::ArithmeticOverflow)?
                };
                i += 1;
            }
            (
                PayoffKindV1::Triangle,
                grid.active_len,
                ApproximationBound::ZERO,
            )
        }
        PayoffSpecV1::CappedLinear {
            grid,
            start,
            end,
            start_amount,
            end_amount,
        } => {
            grid.validate()?;
            if start >= end {
                return Err(Error::InvalidRange);
            }
            let active = usize::from(grid.active_len);
            let mut i = 0_usize;
            while i < active {
                let x = grid.knots[i];
                coefficients[i] = if x <= start {
                    start_amount
                } else if x >= end {
                    end_amount
                } else {
                    lerp_from_start(start_amount, end_amount, x - start, end - start)?
                };
                i += 1;
            }
            (
                PayoffKindV1::CappedLinear,
                grid.active_len,
                ApproximationBound::ZERO,
            )
        }
        PayoffSpecV1::ExactSamples {
            active_len,
            coefficients: samples,
        } => {
            validate_coefficients(active_len, &samples)?;
            coefficients = samples;
            (
                PayoffKindV1::ExactSamples,
                active_len,
                ApproximationBound::ZERO,
            )
        }
        PayoffSpecV1::GaussianApprox {
            grid,
            center,
            sigma,
            height,
        } => {
            grid.validate()?;
            if sigma == 0 {
                return Err(Error::ZeroParameter);
            }
            if center > MAX_GAUSSIAN_COORDINATE || sigma > MAX_GAUSSIAN_COORDINATE {
                return Err(Error::ParameterOutOfRange);
            }
            let active = usize::from(grid.active_len);
            let mut i = 0_usize;
            while i < active {
                if grid.knots[i] > MAX_GAUSSIAN_COORDINATE {
                    return Err(Error::ParameterOutOfRange);
                }
                i += 1;
            }
            let approximation = compile_gaussian(&grid, center, sigma, height, &mut coefficients)?;
            (PayoffKindV1::GaussianApprox, grid.active_len, approximation)
        }
    };

    let compiled = CompiledClaimV1 {
        kind,
        active_len,
        coefficients,
        maximum_payout_atoms: maximum(active_len, &coefficients)?,
        approximation,
    };
    compiled.validate()?;
    Ok(compiled)
}

fn checked_active(active_len: u8) -> Result<usize> {
    let active = usize::from(active_len);
    if active == 0 || active > MAX_OUTCOMES {
        return Err(Error::InvalidOutcomeCount);
    }
    Ok(active)
}

fn validate_coefficients(active_len: u8, coefficients: &[u64; MAX_OUTCOMES]) -> Result<()> {
    let active = checked_active(active_len)?;
    let mut i = active;
    while i < MAX_OUTCOMES {
        if coefficients[i] != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        i += 1;
    }
    Ok(())
}

fn maximum(active_len: u8, values: &[u64; MAX_OUTCOMES]) -> Result<u64> {
    let active = checked_active(active_len)?;
    let mut result = 0_u64;
    let mut i = 0_usize;
    while i < active {
        if values[i] > result {
            result = values[i];
        }
        i += 1;
    }
    Ok(result)
}

fn lerp_from_start(start: u64, end: u64, position: u64, width: u64) -> Result<u64> {
    if width == 0 || position > width {
        return Err(Error::InvalidRange);
    }
    if end >= start {
        let delta = mul_div_floor(
            u128::from(end - start),
            u128::from(position),
            u128::from(width),
        )?;
        u128::from(start)
            .checked_add(delta)
            .ok_or(Error::ArithmeticOverflow)?
            .try_into()
            .map_err(|_| Error::ArithmeticOverflow)
    } else {
        let delta = mul_div_floor(
            u128::from(start - end),
            u128::from(position),
            u128::from(width),
        )?;
        u128::from(start)
            .checked_sub(delta)
            .ok_or(Error::ArithmeticOverflow)?
            .try_into()
            .map_err(|_| Error::ArithmeticOverflow)
    }
}

fn compile_gaussian(
    grid: &KnotGrid,
    center: u64,
    sigma: u64,
    height: u64,
    out: &mut [u64; MAX_OUTCOMES],
) -> Result<ApproximationBound> {
    let sigma_squared = u128::from(sigma)
        .checked_mul(u128::from(sigma))
        .ok_or(Error::ArithmeticOverflow)?;
    let z_denominator = sigma_squared
        .checked_mul(2)
        .ok_or(Error::ArithmeticOverflow)?;
    let active = usize::from(grid.active_len);
    let mut max_knot_error = 0_u128;
    let mut max_interpolation_error = 0_u128;
    let mut max_one_hot_error = 0_u128;
    let mut i = 0_usize;
    while i < active {
        let distance = grid.knots[i].abs_diff(center);
        let z_numerator = u128::from(distance)
            .checked_mul(u128::from(distance))
            .ok_or(Error::ArithmeticOverflow)?;
        let cutoff = z_denominator
            .checked_mul(EXP_CUTOFF)
            .ok_or(Error::ArithmeticOverflow)?;
        let knot_error;
        if z_numerator > cutoff {
            out[i] = 0;
            knot_error = div_ceil(u128::from(height), EXP_CUTOFF_ERROR_DENOMINATOR)?;
        } else if z_numerator == 0 {
            out[i] = height;
            knot_error = 0;
        } else {
            let (lower, upper) = exp_negative_interval(z_numerator, z_denominator)?;
            let coefficient = mul_div_floor(u128::from(height), lower, EXP_SCALE)?;
            out[i] = coefficient
                .try_into()
                .map_err(|_| Error::ArithmeticOverflow)?;
            let interval_width = upper.checked_sub(lower).ok_or(Error::ArithmeticOverflow)?;
            knot_error = div_ceil(
                u128::from(height)
                    .checked_mul(interval_width)
                    .ok_or(Error::ArithmeticOverflow)?,
                EXP_SCALE,
            )?
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        }
        if knot_error > max_knot_error {
            max_knot_error = knot_error;
        }

        if i + 1 < active {
            let gap = grid.knots[i + 1] - grid.knots[i];
            let gap_squared = u128::from(gap)
                .checked_mul(u128::from(gap))
                .ok_or(Error::ArithmeticOverflow)?;
            let numerator = u128::from(height)
                .checked_mul(gap_squared)
                .ok_or(Error::ArithmeticOverflow)?;
            let denominator = sigma_squared
                .checked_mul(8)
                .ok_or(Error::ArithmeticOverflow)?;
            let interpolation_error = div_ceil(numerator, denominator)?;
            if interpolation_error > max_interpolation_error {
                max_interpolation_error = interpolation_error;
            }
            let nearest_radius = u128::from(gap.div_ceil(2));
            let one_hot_error = div_ceil(
                u128::from(height)
                    .checked_mul(nearest_radius)
                    .ok_or(Error::ArithmeticOverflow)?,
                u128::from(sigma),
            )?;
            if one_hot_error > max_one_hot_error {
                max_one_hot_error = one_hot_error;
            }
        }
        i += 1;
    }
    let total_linear_error = max_knot_error
        .checked_add(max_interpolation_error)
        .ok_or(Error::ArithmeticOverflow)?;
    let total_one_hot_error = max_knot_error
        .checked_add(max_one_hot_error)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(ApproximationBound {
        knot_error_atoms: max_knot_error,
        linear_interpolation_error_atoms: max_interpolation_error,
        one_hot_step_error_atoms: max_one_hot_error,
        total_linear_error_atoms: total_linear_error,
        total_one_hot_error_atoms: total_one_hot_error,
    })
}

/// Enclose `exp(-numerator/denominator)` in Q40 fixed point.
fn exp_negative_interval(numerator: u128, denominator: u128) -> Result<(u128, u128)> {
    if denominator == 0 {
        return Err(Error::ZeroParameter);
    }
    let cutoff = denominator
        .checked_mul(EXP_CUTOFF)
        .ok_or(Error::ArithmeticOverflow)?;
    if numerator > cutoff {
        return Err(Error::InvalidRange);
    }

    let mut term_low = EXP_SCALE;
    let mut term_high = EXP_SCALE;
    let mut sum_low = EXP_SCALE;
    let mut sum_high = EXP_SCALE;
    let mut k = 1_u32;
    while k <= EXP_TERMS {
        let term_denominator = denominator
            .checked_mul(u128::from(k))
            .ok_or(Error::ArithmeticOverflow)?;
        term_low = mul_div_floor(term_low, numerator, term_denominator)?;
        term_high = mul_div_ceil(term_high, numerator, term_denominator)?;
        sum_low = sum_low
            .checked_add(term_low)
            .ok_or(Error::ArithmeticOverflow)?;
        sum_high = sum_high
            .checked_add(term_high)
            .ok_or(Error::ArithmeticOverflow)?;
        k += 1;
    }

    let next_denominator = denominator
        .checked_mul(u128::from(EXP_TERMS + 1))
        .ok_or(Error::ArithmeticOverflow)?;
    let next_high = mul_div_ceil(term_high, numerator, next_denominator)?;
    let factor_numerator = denominator
        .checked_mul(u128::from(EXP_TERMS + 2))
        .ok_or(Error::ArithmeticOverflow)?;
    let factor_denominator = factor_numerator
        .checked_sub(numerator)
        .ok_or(Error::ArithmeticOverflow)?;
    let tail_high = mul_div_ceil(next_high, factor_numerator, factor_denominator)?;
    let exp_high = sum_high
        .checked_add(tail_high)
        .ok_or(Error::ArithmeticOverflow)?;

    let square_scale = EXP_SCALE
        .checked_mul(EXP_SCALE)
        .ok_or(Error::ArithmeticOverflow)?;
    let inverse_low = square_scale / exp_high;
    let inverse_high = div_ceil(square_scale, sum_low)?;
    if inverse_low > inverse_high || inverse_high > EXP_SCALE {
        return Err(Error::ArithmeticOverflow);
    }
    Ok((inverse_low, inverse_high))
}

fn mul_div_floor(left: u128, right: u128, denominator: u128) -> Result<u128> {
    if denominator == 0 {
        return Err(Error::ZeroParameter);
    }
    left.checked_mul(right)
        .ok_or(Error::ArithmeticOverflow)
        .map(|product| product / denominator)
}

fn mul_div_ceil(left: u128, right: u128, denominator: u128) -> Result<u128> {
    let product = left.checked_mul(right).ok_or(Error::ArithmeticOverflow)?;
    div_ceil(product, denominator)
}

fn div_ceil(numerator: u128, denominator: u128) -> Result<u128> {
    if denominator == 0 {
        return Err(Error::ZeroParameter);
    }
    let quotient = numerator / denominator;
    if numerator.is_multiple_of(denominator) {
        Ok(quotient)
    } else {
        quotient.checked_add(1).ok_or(Error::ArithmeticOverflow)
    }
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn lcm(left: u64, right: u64) -> Result<u64> {
    if left == 0 || right == 0 {
        return Err(Error::ZeroParameter);
    }
    (left / gcd(left, right))
        .checked_mul(right)
        .ok_or(Error::ArithmeticOverflow)
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(values: &[u64]) -> KnotGrid {
        let mut knots = [0_u64; MAX_OUTCOMES];
        knots[..values.len()].copy_from_slice(values);
        KnotGrid {
            active_len: values.len() as u8,
            knots,
        }
    }

    fn weights(values: &[u64], denominator: u64) -> WeightVector {
        let mut out = [0_u64; MAX_OUTCOMES];
        out[..values.len()].copy_from_slice(values);
        WeightVector {
            active_len: values.len() as u8,
            denominator,
            weights: out,
        }
    }

    #[test]
    fn categorical_range_and_curves_compile_to_expected_exact_vectors() {
        let binary = compile(PayoffSpecV1::Categorical {
            active_len: 2,
            outcome: 1,
            amount: 100,
        })
        .unwrap();
        assert_eq!(&binary.coefficients[..2], &[0, 100]);
        assert_eq!(binary.maximum_payout_atoms, 100);

        let range = compile(PayoffSpecV1::HardRange {
            active_len: 5,
            first: 1,
            end: 4,
            amount: 80,
        })
        .unwrap();
        assert_eq!(&range.coefficients[..5], &[0, 80, 80, 80, 0]);

        let triangle = compile(PayoffSpecV1::Triangle {
            grid: grid(&[0, 10, 20, 30, 40]),
            left: 0,
            peak: 20,
            right: 40,
            height: 100,
        })
        .unwrap();
        assert_eq!(&triangle.coefficients[..5], &[0, 50, 100, 50, 0]);

        let increasing = compile(PayoffSpecV1::CappedLinear {
            grid: grid(&[0, 10, 20, 30, 40]),
            start: 10,
            end: 30,
            start_amount: 20,
            end_amount: 100,
        })
        .unwrap();
        assert_eq!(&increasing.coefficients[..5], &[20, 20, 60, 100, 100]);

        let decreasing = compile(PayoffSpecV1::CappedLinear {
            grid: grid(&[0, 10, 20, 30, 40]),
            start: 10,
            end: 30,
            start_amount: 100,
            end_amount: 20,
        })
        .unwrap();
        assert_eq!(&decreasing.coefficients[..5], &[100, 100, 60, 20, 20]);
    }

    #[test]
    fn exact_samples_are_the_complete_bounded_finite_language() {
        let mut values = [0_u64; MAX_OUTCOMES];
        values[..6].copy_from_slice(&[3, 1, 4, 1, 5, 9]);
        let claim = compile(PayoffSpecV1::ExactSamples {
            active_len: 6,
            coefficients: values,
        })
        .unwrap();
        assert_eq!(claim.coefficients, values);
        assert_eq!(claim.maximum_payout_atoms, 9);
        assert_eq!(claim.approximation, ApproximationBound::ZERO);

        // ExactSamples does not privilege an analytic vocabulary: this table
        // is a quadratic's exact values at four anchors, while the second is
        // an arbitrary nonnegative tabulated kernel. Both enter settlement as
        // the same bounded coefficient type.
        let mut polynomial = [0_u64; MAX_OUTCOMES];
        polynomial[..4].copy_from_slice(&[0, 1, 4, 9]);
        let quadratic = compile(PayoffSpecV1::ExactSamples {
            active_len: 4,
            coefficients: polynomial,
        })
        .unwrap();
        assert_eq!(&quadratic.coefficients[..4], &[0, 1, 4, 9]);

        let mut kernel = [0_u64; MAX_OUTCOMES];
        kernel[..5].copy_from_slice(&[2, 7, 11, 7, 2]);
        let tabulated = compile(PayoffSpecV1::ExactSamples {
            active_len: 5,
            coefficients: kernel,
        })
        .unwrap();
        assert_eq!(tabulated.maximum_payout_atoms, 11);
    }

    #[test]
    fn malformed_shape_and_padding_refuse() {
        assert_eq!(
            compile(PayoffSpecV1::Constant {
                active_len: 0,
                amount: 1,
            }),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            compile(PayoffSpecV1::HardRange {
                active_len: 2,
                first: 1,
                end: 3,
                amount: 1,
            }),
            Err(Error::InvalidRange)
        );
        let mut noncanonical = [0_u64; MAX_OUTCOMES];
        noncanonical[2] = 1;
        assert_eq!(
            compile(PayoffSpecV1::ExactSamples {
                active_len: 2,
                coefficients: noncanonical,
            }),
            Err(Error::NonCanonicalPadding)
        );
        assert_eq!(grid(&[1, 1]).validate(), Err(Error::InvalidKnotGrid));
        assert_eq!(
            compile(PayoffSpecV1::GaussianApprox {
                grid: grid(&[0, MAX_GAUSSIAN_COORDINATE + 1]),
                center: 0,
                sigma: 1,
                height: 1,
            }),
            Err(Error::ParameterOutOfRange)
        );

        let mut forged_certificate = compile(PayoffSpecV1::Constant {
            active_len: 2,
            amount: 1,
        })
        .unwrap();
        forged_certificate.approximation.knot_error_atoms = 1;
        forged_certificate.approximation.total_linear_error_atoms = 1;
        forged_certificate.approximation.total_one_hot_error_atoms = 1;
        assert_eq!(forged_certificate.validate(), Err(Error::InvalidRange));
    }

    #[test]
    fn gaussian_compiler_is_symmetric_monotone_and_certified() {
        let claim = compile(PayoffSpecV1::GaussianApprox {
            grid: grid(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
            center: 5,
            sigma: 1,
            height: 1_000_000,
        })
        .unwrap();
        assert_eq!(claim.coefficients[5], 1_000_000);
        let mut i = 0_usize;
        while i < 5 {
            assert_eq!(claim.coefficients[i], claim.coefficients[10 - i]);
            assert!(claim.coefficients[i] <= claim.coefficients[i + 1]);
            i += 1;
        }
        assert!((600_000..=610_000).contains(&claim.coefficients[4]));
        assert!((130_000..=140_000).contains(&claim.coefficients[3]));
        assert!((300..=400).contains(&claim.coefficients[1]));
        assert_eq!(claim.coefficients[0], 0);
        assert!(claim.approximation.knot_error_atoms >= 489);
        assert_eq!(
            claim.approximation.linear_interpolation_error_atoms,
            125_000
        );
        assert_eq!(claim.approximation.one_hot_step_error_atoms, 1_000_000);
        claim.validate().unwrap();
    }

    #[test]
    fn one_hot_primitive_eggs_make_every_integer_portfolio_atom_exact() {
        let mut samples = [0_u64; MAX_OUTCOMES];
        samples[..4].copy_from_slice(&[0, 3, 7, 2]);
        let claim = compile(PayoffSpecV1::ExactSamples {
            active_len: 4,
            coefficients: samples,
        })
        .unwrap();
        let mut lots = 1_u64;
        while lots <= 7 {
            let mut outcome = 0_u8;
            while outcome < 4 {
                let payout = claim.one_hot_payout(lots, outcome).unwrap();
                assert_eq!(payout, u128::from(lots * samples[usize::from(outcome)]));
                assert!(payout <= claim.worst_case_liability(lots).unwrap());
                outcome += 1;
            }
            lots += 1;
        }

        // The same integer portfolio over native fractional primitive Eggs can
        // require aggregation: (0*3 + 3*5)/8 is not an atom count.
        let mut fractional_samples = [0_u64; MAX_OUTCOMES];
        fractional_samples[..2].copy_from_slice(&[0, 3]);
        let fractional = compile(PayoffSpecV1::ExactSamples {
            active_len: 2,
            coefficients: fractional_samples,
        })
        .unwrap();
        let native_fractional_weights = weights(&[3, 5], 8);
        assert_eq!(
            fractional.exact_payout(1, &native_fractional_weights),
            Err(Error::RemainderRequired)
        );
        assert_eq!(fractional.universal_exact_lot(8).unwrap(), 8);
        fractional
            .exact_payout(8, &native_fractional_weights)
            .unwrap();
    }

    #[test]
    fn positive_taylor_interval_pins_known_rational_cases() {
        assert_eq!(exp_negative_interval(0, 1).unwrap(), (EXP_SCALE, EXP_SCALE));
        let (half_low, half_high) = exp_negative_interval(1, 2).unwrap();
        assert!(half_low <= half_high);
        assert!(half_low > EXP_SCALE * 60 / 100);
        assert!(half_high < EXP_SCALE * 61 / 100);
        let (two_low, two_high) = exp_negative_interval(2, 1).unwrap();
        assert!(two_low > EXP_SCALE * 13 / 100);
        assert!(two_high < EXP_SCALE * 14 / 100);
    }

    #[test]
    fn full_simplex_liability_and_universal_lot_hold_exhaustively() {
        let mut checked = 0_u64;
        let denominator = 6_u64;
        let mut a = 0_u64;
        while a <= 3 {
            let mut b = 0_u64;
            while b <= 3 {
                let mut c = 0_u64;
                while c <= 3 {
                    let mut samples = [0_u64; MAX_OUTCOMES];
                    samples[..3].copy_from_slice(&[a, b, c]);
                    let claim = compile(PayoffSpecV1::ExactSamples {
                        active_len: 3,
                        coefficients: samples,
                    })
                    .unwrap();
                    let lot = claim.universal_exact_lot(denominator).unwrap();
                    assert!(lot >= 1 && lot <= denominator);
                    let mut x = 0_u64;
                    while x <= denominator {
                        let mut y = 0_u64;
                        while y <= denominator - x {
                            let z = denominator - x - y;
                            let vector = weights(&[x, y, z], denominator);
                            vector.validate().unwrap();
                            let ceiling = claim.ceiling_payout(1, &vector).unwrap();
                            assert!(ceiling <= u128::from(claim.maximum_payout_atoms));
                            let exact = claim.exact_payout(lot, &vector).unwrap();
                            assert!(
                                exact <= claim.worst_case_liability(lot).unwrap(),
                                "a={a} b={b} c={c} w=({x},{y},{z}) lot={lot}"
                            );
                            checked += 1;
                            y += 1;
                        }
                        x += 1;
                    }

                    // Minimality, not just sufficiency: every smaller lot is
                    // exposed by at least one full-simplex weight vector.
                    let mut candidate = 1_u64;
                    while candidate < lot {
                        let mut exposes_remainder = false;
                        let mut x = 0_u64;
                        while x <= denominator {
                            let mut y = 0_u64;
                            while y <= denominator - x {
                                let z = denominator - x - y;
                                let vector = weights(&[x, y, z], denominator);
                                if claim.exact_payout(candidate, &vector)
                                    == Err(Error::RemainderRequired)
                                {
                                    exposes_remainder = true;
                                }
                                y += 1;
                            }
                            x += 1;
                        }
                        assert!(
                            exposes_remainder,
                            "a={a} b={b} c={c} candidate={candidate} reported={lot}"
                        );
                        candidate += 1;
                    }
                    c += 1;
                }
                b += 1;
            }
            a += 1;
        }
        assert_eq!(checked, 1_792);
    }

    #[test]
    fn universal_lot_is_least_for_the_full_simplex() {
        let mut samples = [0_u64; MAX_OUTCOMES];
        samples[..3].copy_from_slice(&[1, 3, 4]);
        let claim = compile(PayoffSpecV1::ExactSamples {
            active_len: 3,
            coefficients: samples,
        })
        .unwrap();
        assert_eq!(claim.universal_exact_lot(6).unwrap(), 6);
        let exposing_weight = weights(&[5, 1, 0], 6);
        let mut smaller = 1_u64;
        while smaller < 6 {
            if !smaller.is_multiple_of(3) {
                assert_eq!(
                    claim.exact_payout(smaller, &exposing_weight),
                    Err(Error::RemainderRequired)
                );
            }
            smaller += 1;
        }
        claim.exact_payout(6, &exposing_weight).unwrap();
    }

    #[test]
    fn invalid_weight_partitions_refuse() {
        assert_eq!(weights(&[2, 3], 6).validate(), Err(Error::InvalidWeights));
        let mut padded = weights(&[2, 4], 6);
        padded.weights[2] = 1;
        assert_eq!(padded.validate(), Err(Error::NonCanonicalPadding));
    }
}
