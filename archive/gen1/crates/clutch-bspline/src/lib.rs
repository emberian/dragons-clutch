#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Exact, bounded B-spline payout-basis evaluation.
//!
//! The frozen representation contains distinct breakpoints. Degrees two and
//! three expand them to the standard open-clamped knot vector by repeating
//! each endpoint `degree + 1` times and keeping each interior breakpoint once.
//! Consequently `K` distinct breakpoints produce `K - 1 + degree` claims.
//! This is the representation frozen by
//! `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md`.
//!
//! Weight quantization uses deterministic largest remainder: floor every
//! exact scaled basis value, then distribute the remaining at-most-`degree`
//! atoms to the largest fractional remainders, breaking exact ties by lowest
//! outcome index. Earlier host-only degree-one code assigned every residual
//! atom to the lower outcome. It had no deployed compatibility authority and
//! is intentionally not preserved: at coarse denominators it could assign a
//! whole atom to a basis function with arbitrarily small exact mass.

/// Maximum number of payout claims in one market.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum number of distinct stored knots or degree-zero boundaries.
pub const MAX_KNOTS: usize = 16;
/// Largest implemented B-spline degree.
pub const MAX_DEGREE: u8 = 3;
/// Sentinel for a non-uniform knot declaration.
pub const UNIFORM_SPACING_NONE: u8 = u8::MAX;

const MAX_LOCAL_BASIS: usize = 4;
#[cfg(test)]
const MAX_EXPANDED_KNOTS: usize = MAX_KNOTS + (2 * MAX_DEGREE as usize);

/// Frozen handling of values outside a derived basis's knot span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgePolicy {
    /// Clamp to the closest endpoint, whose extreme claim receives full weight.
    Clamp,
    /// Refuse a value outside the knot span.
    Refuse,
}

/// A canonical refusal from basis admission or evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The active outcome count is outside `2..=MAX_OUTCOMES`.
    InvalidOutcomeCount,
    /// The degree is outside `0..=MAX_DEGREE`.
    InvalidDegree,
    /// The common integer weight denominator is zero.
    InvalidDenominator,
    /// The knot count does not match the degree/outcome count relation.
    InvalidKnotCount,
    /// Active knots are unordered, outside the admitted domain, or deg-0 starts at zero.
    InvalidKnot,
    /// An inactive knot entry is not canonical zero padding.
    NonCanonicalPadding,
    /// A uniform-spacing declaration is malformed or contradicts the knots.
    UniformSpacingMismatch,
    /// A smooth basis omitted the mandatory uniform power-of-two declaration.
    UniformSpacingRequired,
    /// A degree-zero categorical partition selected a non-clamping edge policy.
    InvalidEdgePolicy,
    /// Freeze-time bounds do not prove every runtime product fits in `u128`.
    ArithmeticBound,
    /// The refusing edge policy rejected a value outside the knot span.
    ValueOutOfRange,
    /// A checked runtime operation overflowed or an exact denominator vanished.
    ArithmeticOverflow,
    /// A constructed vector violated its denominator, support, or padding invariant.
    InvalidWeights,
}

/// Result alias for exact basis operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Hostile-input-facing frozen basis description.
///
/// Direct operations validate the whole value before using it; callers doing
/// repeated evaluations may explicitly obtain a [`ValidatedBasisSpec`]. For
/// degree zero, `knots[0..knot_count]` are interior cell boundaries in
/// `(0, domain_max]`, and `outcome_count = knot_count + 1`. For degree at
/// least one, they are distinct breakpoints/anchors and
/// `outcome_count = knot_count - 1 + degree`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisSpec {
    /// Number of active claims.
    pub outcome_count: u8,
    /// B-spline degree in `0..=3`.
    pub degree: u8,
    /// Number of active entries in `knots`.
    pub knot_count: u8,
    /// `s` for spacing `2^s`, or [`UNIFORM_SPACING_NONE`].
    pub uniform_log2_spacing: u8,
    /// Positive integer payout-weight denominator `D`.
    pub denominator: u64,
    /// Largest admitted resolved coordinate.
    pub domain_max: u128,
    /// Frozen out-of-span behavior.
    pub edge_policy: EdgePolicy,
    /// Active knot/boundary prefix followed by zero padding.
    pub knots: [u128; MAX_KNOTS],
}

/// Validated capability for repeated evaluation of one immutable basis.
///
/// Its fields are private and the only constructor validates the complete
/// hostile [`BasisSpec`]. Reusing this value therefore removes redundant shape
/// validation without introducing an unchecked public precondition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedBasisSpec {
    spec: BasisSpec,
    smooth_common_shift: u8,
    smooth_common_odd: u8,
}

impl BasisSpec {
    /// Validate shape, canonical padding, spacing, and freeze-time arithmetic bounds.
    pub fn validate(&self) -> Result<()> {
        let degree = usize::from(self.degree);
        if self.degree > MAX_DEGREE {
            return Err(Error::InvalidDegree);
        }
        let outcomes = usize::from(self.outcome_count);
        if !(2..=MAX_OUTCOMES).contains(&outcomes) {
            return Err(Error::InvalidOutcomeCount);
        }
        if self.denominator == 0 {
            return Err(Error::InvalidDenominator);
        }
        let knot_count = usize::from(self.knot_count);
        let expected_knots = if degree == 0 {
            outcomes - 1
        } else {
            outcomes
                .checked_add(1)
                .and_then(|value| value.checked_sub(degree))
                .ok_or(Error::InvalidKnotCount)?
        };
        if knot_count == 0 || knot_count > MAX_KNOTS || knot_count != expected_knots {
            return Err(Error::InvalidKnotCount);
        }
        if degree >= 1 && knot_count < 2 {
            return Err(Error::InvalidKnotCount);
        }
        if degree == 0 && self.edge_policy != EdgePolicy::Clamp {
            return Err(Error::InvalidEdgePolicy);
        }

        let mut largest_gap = 0_u128;
        let mut index = 0_usize;
        while index < MAX_KNOTS {
            let knot = self.knots[index];
            if index < knot_count {
                if knot > self.domain_max {
                    return Err(Error::InvalidKnot);
                }
                if index == 0 {
                    if degree == 0 && knot == 0 {
                        return Err(Error::InvalidKnot);
                    }
                } else {
                    let previous = self.knots[index - 1];
                    if knot <= previous {
                        return Err(Error::InvalidKnot);
                    }
                    let gap = knot - previous;
                    if gap > largest_gap {
                        largest_gap = gap;
                    }
                }
            } else if knot != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }

        if self.uniform_log2_spacing == UNIFORM_SPACING_NONE {
            if degree >= 2 {
                return Err(Error::UniformSpacingRequired);
            }
        } else {
            if self.uniform_log2_spacing >= 128 {
                return Err(Error::UniformSpacingMismatch);
            }
            let gap = 1_u128 << self.uniform_log2_spacing;
            let mut index = 1_usize;
            while index < knot_count {
                if self.knots[index] - self.knots[index - 1] != gap {
                    return Err(Error::UniformSpacingMismatch);
                }
                index += 1;
            }
        }

        if degree >= 1 {
            // RECURRENCE-BOUND-01. A uniform degree-two pane is exactly
            // representable over `2*h^2`; every open-clamped degree-three
            // pane, including overlapping edge effects on short grids, is
            // exactly representable over `12*h^3`. The fixed recurrence
            // cancels every `h`, `2h`, or `3h` divisor before multiplying and
            // checks that cancellation. Nonnegative partial basis sums remain
            // at most one, so their common-denominator numerators and checked
            // additions remain at most that common denominator. The admitted
            // operand is `2*h^2` or `6*h^3`; multiplying it by D below 2^127
            // therefore proves the quadratic scaled common bound directly and
            // leaves exactly the factor-two margin needed for the cubic
            // `12*h^3 * D` bound below 2^128. Near-limit accepted/refused tests
            // and the test-only Fraction differential pin both boundaries.
            let operand = match degree {
                1 => largest_gap.checked_sub(1).ok_or(Error::ArithmeticBound)?,
                2 => largest_gap
                    .checked_mul(largest_gap)
                    .and_then(|value| value.checked_mul(2))
                    .ok_or(Error::ArithmeticBound)?,
                3 => largest_gap
                    .checked_mul(largest_gap)
                    .and_then(|value| value.checked_mul(largest_gap))
                    .and_then(|value| value.checked_mul(6))
                    .ok_or(Error::ArithmeticBound)?,
                _ => return Err(Error::InvalidDegree),
            };
            let bound = u128::from(self.denominator)
                .checked_mul(operand)
                .ok_or(Error::ArithmeticBound)?;
            if bound >> 127 != 0 {
                return Err(Error::ArithmeticBound);
            }
        }
        Ok(())
    }

    /// Evaluate this basis at one integer resolved coordinate.
    ///
    /// The returned vector always has exact sum `D`, zero padding, and support
    /// at most `degree + 1`. The top coordinate is closed: it returns the last
    /// claim at full weight rather than an all-zero half-open evaluation.
    pub fn evaluate(&self, value: u128) -> Result<WeightVector> {
        self.validated()?.evaluate_point(value)
    }

    /// Validate once and return a capability suitable for repeated evaluation.
    pub fn validated(&self) -> Result<ValidatedBasisSpec> {
        self.validate()?;
        let (smooth_common_shift, smooth_common_odd) = match self.degree {
            2 => uniform_common_parameters(self.uniform_log2_spacing, 2)?,
            3 => uniform_common_parameters(self.uniform_log2_spacing, 12)?,
            _ => (0, 0),
        };
        Ok(ValidatedBasisSpec {
            spec: *self,
            smooth_common_shift,
            smooth_common_odd,
        })
    }
}

impl ValidatedBasisSpec {
    /// Return the exact immutable basis captured by this capability.
    pub const fn spec(self) -> BasisSpec {
        self.spec
    }

    /// Evaluate one coordinate under the already validated immutable basis.
    pub fn evaluate_point(&self, value: u128) -> Result<WeightVector> {
        let spec = &self.spec;
        if spec.degree == 0 {
            return spec.evaluate_degree_zero(value);
        }
        let handled = spec.handle_edge(value)?;
        let knot_count = usize::from(spec.knot_count);
        let first = spec.knots[0];
        let last = spec.knots[knot_count - 1];
        if handled <= first {
            return WeightVector::one_hot(spec.outcome_count, spec.denominator, 0);
        }
        if handled >= last {
            return WeightVector::one_hot(
                spec.outcome_count,
                spec.denominator,
                usize::from(spec.outcome_count) - 1,
            );
        }
        let pane = spec.locate_pane(handled)?;
        if spec.degree == 1 {
            spec.evaluate_degree_one(handled, pane)
        } else {
            self.evaluate_smooth(handled, pane)
        }
    }
}

impl BasisSpec {
    fn evaluate_degree_zero(&self, value: u128) -> Result<WeightVector> {
        let outcomes = usize::from(self.outcome_count);
        let mut cell = 0_usize;
        while cell + 1 < outcomes && value >= self.knots[cell] {
            cell += 1;
        }
        WeightVector::one_hot(self.outcome_count, self.denominator, cell)
    }

    fn handle_edge(&self, value: u128) -> Result<u128> {
        let last = self.knots[usize::from(self.knot_count) - 1];
        let first = self.knots[0];
        match self.edge_policy {
            EdgePolicy::Clamp => Ok(value.clamp(first, last)),
            EdgePolicy::Refuse => {
                if value < first || value > last {
                    Err(Error::ValueOutOfRange)
                } else {
                    Ok(value)
                }
            }
        }
    }

    fn locate_pane(&self, value: u128) -> Result<usize> {
        let knots = usize::from(self.knot_count);
        if self.uniform_log2_spacing != UNIFORM_SPACING_NONE {
            let pane_u128 = (value - self.knots[0]) >> self.uniform_log2_spacing;
            let pane = usize::try_from(pane_u128).map_err(|_| Error::ArithmeticOverflow)?;
            if pane + 1 >= knots {
                return Err(Error::ArithmeticOverflow);
            }
            return Ok(pane);
        }
        let mut pane = 0_usize;
        while pane + 2 < knots && value >= self.knots[pane + 1] {
            pane += 1;
        }
        Ok(pane)
    }

    /// Division/shift specialization of WEIGHT-ROUND-01 for degree one.
    fn evaluate_degree_one(&self, value: u128, pane: usize) -> Result<WeightVector> {
        let lower = self.knots[pane];
        let gap = self.knots[pane + 1] - lower;
        let offset = value - lower;
        let denominator = self.denominator;
        let product = u128::from(denominator)
            .checked_mul(offset)
            .ok_or(Error::ArithmeticOverflow)?;
        let (right_floor, right_remainder) = if self.uniform_log2_spacing != UNIFORM_SPACING_NONE {
            let shift = self.uniform_log2_spacing;
            let floor = product >> shift;
            let remainder = product & (gap - 1);
            (floor, remainder)
        } else {
            (product / gap, product % gap)
        };
        let mut right = u64::try_from(right_floor).map_err(|_| Error::ArithmeticOverflow)?;
        if right > denominator {
            return Err(Error::InvalidWeights);
        }
        // The left and right fractional remainders are complementary whenever
        // the division is inexact. One residual atom goes to the larger; the
        // lowest outcome (left) wins an exact half tie.
        if right_remainder != 0 {
            let left_remainder = gap - right_remainder;
            if right_remainder > left_remainder {
                right = right.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
        }
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[pane] = denominator - right;
        weights[pane + 1] = right;
        WeightVector::checked(self.outcome_count, denominator, weights, 2)
    }

    #[cfg(test)]
    fn evaluate_smooth_fraction_oracle(&self, value: u128, pane: usize) -> Result<WeightVector> {
        let degree = usize::from(self.degree);
        let expanded = self.expanded_knots()?;
        let span = degree.checked_add(pane).ok_or(Error::ArithmeticOverflow)?;
        let exact = basis_functions(span, degree, value, &expanded)?;
        let mut weights = [0_u64; MAX_OUTCOMES];
        let mut remainders = [Fraction::ZERO; MAX_LOCAL_BASIS];
        let mut floor_sum = 0_u64;
        let mut local = 0_usize;
        while local <= degree {
            let (floor, remainder) = exact[local].scaled_parts(self.denominator)?;
            weights[pane + local] = floor;
            remainders[local] = remainder;
            floor_sum = floor_sum
                .checked_add(floor)
                .ok_or(Error::ArithmeticOverflow)?;
            local += 1;
        }
        let residual = self
            .denominator
            .checked_sub(floor_sum)
            .ok_or(Error::InvalidWeights)?;
        if residual > self.degree.into() {
            return Err(Error::InvalidWeights);
        }
        let mut awarded = [false; MAX_LOCAL_BASIS];
        let mut remaining = residual;
        while remaining > 0 {
            let mut best: Option<usize> = None;
            let mut index = 0_usize;
            while index <= degree {
                if !awarded[index] && remainders[index] != Fraction::ZERO {
                    let replace = match best {
                        None => true,
                        Some(current) => {
                            // Strict greater only: scanning low to high makes
                            // the lowest outcome the deterministic exact-tie
                            // winner without a second ordering rule.
                            remainders[index].compare(remainders[current])?
                                == core::cmp::Ordering::Greater
                        }
                    };
                    if replace {
                        best = Some(index);
                    }
                }
                index += 1;
            }
            let selected = best.ok_or(Error::InvalidWeights)?;
            weights[pane + selected] = weights[pane + selected]
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
            awarded[selected] = true;
            remaining -= 1;
        }
        WeightVector::checked(self.outcome_count, self.denominator, weights, degree + 1)
    }

    #[cfg(test)]
    fn expanded_knots(&self) -> Result<[u128; MAX_EXPANDED_KNOTS]> {
        let degree = usize::from(self.degree);
        let knot_count = usize::from(self.knot_count);
        let mut expanded = [0_u128; MAX_EXPANDED_KNOTS];
        let mut cursor = 0_usize;
        let mut repeat = 0_usize;
        while repeat <= degree {
            expanded[cursor] = self.knots[0];
            cursor += 1;
            repeat += 1;
        }
        let mut index = 1_usize;
        while index + 1 < knot_count {
            expanded[cursor] = self.knots[index];
            cursor += 1;
            index += 1;
        }
        repeat = 0;
        while repeat <= degree {
            expanded[cursor] = self.knots[knot_count - 1];
            cursor += 1;
            repeat += 1;
        }
        let expected = knot_count
            .checked_add(2 * degree)
            .ok_or(Error::ArithmeticOverflow)?;
        if cursor != expected || cursor > MAX_EXPANDED_KNOTS {
            return Err(Error::ArithmeticOverflow);
        }
        Ok(expanded)
    }
}

impl ValidatedBasisSpec {
    /// Fixed-denominator specialization of the exact Cox--de Boor recurrence.
    ///
    /// For uniform integer spacing `h`, every quadratic intermediate is
    /// representable over `2*h^2`; every cubic intermediate is representable
    /// over `12*h^3`. The latter deliberately includes both `3*h` and the
    /// repeated half factors that meet at clamped/internal panes. Each product
    /// cancels its divisor before multiplying, so the same freeze-time bound
    /// that admitted the basis also bounds every runtime intermediate.
    fn evaluate_smooth(&self, value: u128, pane: usize) -> Result<WeightVector> {
        let spec = &self.spec;
        let degree = usize::from(spec.degree);
        let span = degree.checked_add(pane).ok_or(Error::ArithmeticOverflow)?;
        let common = u128::from(self.smooth_common_odd)
            .checked_shl(u32::from(self.smooth_common_shift))
            .ok_or(Error::ArithmeticOverflow)?;
        let mut basis = [0_u128; MAX_LOCAL_BASIS];
        let mut left = [0_u128; MAX_LOCAL_BASIS];
        let mut right = [0_u128; MAX_LOCAL_BASIS];
        basis[0] = common;
        let mut column = 1_usize;
        while column <= degree {
            left[column] = value
                .checked_sub(self.expanded_knot(span + 1 - column)?)
                .ok_or(Error::ArithmeticOverflow)?;
            right[column] = self
                .expanded_knot(span + column)?
                .checked_sub(value)
                .ok_or(Error::ArithmeticOverflow)?;
            let mut saved = 0_u128;
            let mut row = 0_usize;
            while row < column {
                let divisor = right[row + 1]
                    .checked_add(left[column - row])
                    .ok_or(Error::ArithmeticOverflow)?;
                let right_term = mul_div_exact(basis[row], right[row + 1], divisor)?;
                let left_term = mul_div_exact(basis[row], left[column - row], divisor)?;
                basis[row] = saved
                    .checked_add(right_term)
                    .ok_or(Error::ArithmeticOverflow)?;
                saved = left_term;
                row += 1;
            }
            basis[column] = saved;
            column += 1;
        }

        let mut weights = [0_u64; MAX_OUTCOMES];
        let mut remainders = [0_u128; MAX_LOCAL_BASIS];
        let mut floor_sum = 0_u64;
        let mut local = 0_usize;
        while local <= degree {
            let scaled = basis[local]
                .checked_mul(u128::from(spec.denominator))
                .ok_or(Error::ArithmeticOverflow)?;
            let (floor_u128, remainder) =
                scaled_parts_common(scaled, self.smooth_common_shift, self.smooth_common_odd)?;
            let floor = u64::try_from(floor_u128).map_err(|_| Error::ArithmeticOverflow)?;
            weights[pane + local] = floor;
            remainders[local] = remainder;
            floor_sum = floor_sum
                .checked_add(floor)
                .ok_or(Error::ArithmeticOverflow)?;
            local += 1;
        }
        let residual = spec
            .denominator
            .checked_sub(floor_sum)
            .ok_or(Error::InvalidWeights)?;
        if residual > spec.degree.into() {
            return Err(Error::InvalidWeights);
        }
        let mut awarded = [false; MAX_LOCAL_BASIS];
        let mut remaining = residual;
        while remaining > 0 {
            let mut best: Option<usize> = None;
            let mut index = 0_usize;
            while index <= degree {
                if !awarded[index] && remainders[index] != 0 {
                    let replace = match best {
                        None => true,
                        Some(current) => remainders[index] > remainders[current],
                    };
                    if replace {
                        best = Some(index);
                    }
                }
                index += 1;
            }
            let selected = best.ok_or(Error::InvalidWeights)?;
            weights[pane + selected] = weights[pane + selected]
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
            awarded[selected] = true;
            remaining -= 1;
        }
        WeightVector::checked(spec.outcome_count, spec.denominator, weights, degree + 1)
    }

    fn expanded_knot(&self, index: usize) -> Result<u128> {
        let spec = &self.spec;
        let degree = usize::from(spec.degree);
        let knot_count = usize::from(spec.knot_count);
        let expanded_len = knot_count
            .checked_add(2 * degree)
            .ok_or(Error::ArithmeticOverflow)?;
        if index >= expanded_len {
            return Err(Error::ArithmeticOverflow);
        }
        if index <= degree {
            return Ok(spec.knots[0]);
        }
        let interior_end = degree
            .checked_add(knot_count - 1)
            .ok_or(Error::ArithmeticOverflow)?;
        if index < interior_end {
            return Ok(spec.knots[index - degree]);
        }
        Ok(spec.knots[knot_count - 1])
    }
}

fn uniform_common_parameters(spacing_shift: u8, degree_coefficient: u8) -> Result<(u8, u8)> {
    let (common_shift, odd) = match degree_coefficient {
        2 => (u16::from(spacing_shift) * 2 + 1, 1_u8),
        12 => (u16::from(spacing_shift) * 3 + 2, 3_u8),
        _ => return Err(Error::ArithmeticOverflow),
    };
    let common_shift = u8::try_from(common_shift).map_err(|_| Error::ArithmeticOverflow)?;
    u128::from(odd)
        .checked_shl(u32::from(common_shift))
        .ok_or(Error::ArithmeticOverflow)?;
    Ok((common_shift, odd))
}

fn mul_div_exact(numerator: u128, factor: u128, divisor: u128) -> Result<u128> {
    if divisor == 0 {
        return Err(Error::ArithmeticOverflow);
    }
    if numerator == 0 || factor == 0 {
        return Ok(0);
    }
    // Every divisor in the validated uniform recurrence is h, 2h, or 3h
    // with power-of-two h. Cancel its power-of-two component by shifts, then
    // its only possible odd factor (3) from either multiplicand. This is the
    // exact reduced multiplication the Fraction oracle performs, without a
    // variable-width Euclidean GCD in production.
    let divisor_shift = divisor.trailing_zeros();
    let factor_shift = factor.trailing_zeros().min(divisor_shift);
    let reduced_factor = factor >> factor_shift;
    let remaining_shift = divisor_shift - factor_shift;
    if numerator.trailing_zeros() < remaining_shift {
        return Err(Error::ArithmeticOverflow);
    }
    let mut reduced_numerator = numerator >> remaining_shift;
    let odd = divisor >> divisor_shift;
    let mut reduced_factor = reduced_factor;
    if odd == 3 {
        let factor_quotient = reduced_factor / 3;
        if factor_quotient * 3 == reduced_factor {
            reduced_factor = factor_quotient;
        } else {
            let numerator_quotient = reduced_numerator / 3;
            if numerator_quotient * 3 != reduced_numerator {
                return Err(Error::ArithmeticOverflow);
            }
            reduced_numerator = numerator_quotient;
        }
    } else if odd != 1 {
        return Err(Error::ArithmeticOverflow);
    }
    reduced_numerator
        .checked_mul(reduced_factor)
        .ok_or(Error::ArithmeticOverflow)
}

fn scaled_parts_common(scaled: u128, shift: u8, odd: u8) -> Result<(u128, u128)> {
    let shift = u32::from(shift);
    let mask = 1_u128
        .checked_shl(shift)
        .and_then(|power| power.checked_sub(1))
        .ok_or(Error::ArithmeticOverflow)?;
    let low = scaled & mask;
    let upper = scaled >> shift;
    match odd {
        1 => Ok((upper, low)),
        3 => {
            let quotient = upper / 3;
            let remainder = upper - quotient * 3;
            Ok((quotient, (remainder << shift) | low))
        }
        _ => Err(Error::ArithmeticOverflow),
    }
}

/// Exact integer partition-of-unity payout weights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeightVector {
    /// Number of active weights.
    pub active_len: u8,
    /// Positive common denominator.
    pub denominator: u64,
    /// Active weights summing exactly to `denominator`, then zero padding.
    pub weights: [u64; MAX_OUTCOMES],
}

impl WeightVector {
    fn one_hot(active_len: u8, denominator: u64, index: usize) -> Result<Self> {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[index] = denominator;
        Self::checked(active_len, denominator, weights, 1)
    }

    fn checked(
        active_len: u8,
        denominator: u64,
        weights: [u64; MAX_OUTCOMES],
        support_bound: usize,
    ) -> Result<Self> {
        let value = Self {
            active_len,
            denominator,
            weights,
        };
        value.validate_with_support(support_bound)?;
        Ok(value)
    }

    /// Validate shape, bounds, canonical padding, and exact partition unity.
    pub fn validate(&self) -> Result<()> {
        self.validate_with_support(MAX_LOCAL_BASIS)
    }

    fn validate_with_support(&self, support_bound: usize) -> Result<()> {
        let active = usize::from(self.active_len);
        if !(2..=MAX_OUTCOMES).contains(&active) {
            return Err(Error::InvalidOutcomeCount);
        }
        if self.denominator == 0 {
            return Err(Error::InvalidDenominator);
        }
        let mut sum = 0_u128;
        let mut support = 0_usize;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let weight = self.weights[index];
            if index < active {
                if weight > self.denominator {
                    return Err(Error::InvalidWeights);
                }
                sum = sum
                    .checked_add(u128::from(weight))
                    .ok_or(Error::ArithmeticOverflow)?;
                if weight != 0 {
                    support += 1;
                }
            } else if weight != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if sum != u128::from(self.denominator) || support > support_bound {
            return Err(Error::InvalidWeights);
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Fraction {
    numerator: u128,
    denominator: u128,
}

#[cfg(test)]
impl Fraction {
    const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };
    const ONE: Self = Self {
        numerator: 1,
        denominator: 1,
    };

    fn new(numerator: u128, denominator: u128) -> Result<Self> {
        if denominator == 0 {
            return Err(Error::ArithmeticOverflow);
        }
        if numerator == 0 {
            return Ok(Self::ZERO);
        }
        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    fn add(self, other: Self) -> Result<Self> {
        let common = gcd(self.denominator, other.denominator);
        let self_factor = other.denominator / common;
        let other_factor = self.denominator / common;
        let numerator = self
            .numerator
            .checked_mul(self_factor)
            .and_then(|left| {
                other
                    .numerator
                    .checked_mul(other_factor)
                    .and_then(|right| left.checked_add(right))
            })
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = self
            .denominator
            .checked_mul(self_factor)
            .ok_or(Error::ArithmeticOverflow)?;
        Self::new(numerator, denominator)
    }

    fn multiply_integer(self, value: u128) -> Result<Self> {
        if self.numerator == 0 || value == 0 {
            return Ok(Self::ZERO);
        }
        let common = gcd(value, self.denominator);
        let numerator = self
            .numerator
            .checked_mul(value / common)
            .ok_or(Error::ArithmeticOverflow)?;
        Self::new(numerator, self.denominator / common)
    }

    fn divide_integer(self, value: u128) -> Result<Self> {
        if value == 0 {
            return Err(Error::ArithmeticOverflow);
        }
        if self.numerator == 0 {
            return Ok(Self::ZERO);
        }
        let common = gcd(self.numerator, value);
        let denominator = self
            .denominator
            .checked_mul(value / common)
            .ok_or(Error::ArithmeticOverflow)?;
        Self::new(self.numerator / common, denominator)
    }

    /// Split `scale * self` into its integer floor and exact fractional part.
    fn scaled_parts(self, scale: u64) -> Result<(u64, Self)> {
        let scale = u128::from(scale);
        let common = gcd(scale, self.denominator);
        let numerator = self
            .numerator
            .checked_mul(scale / common)
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = self.denominator / common;
        let quotient = numerator / denominator;
        let remainder = Self::new(numerator % denominator, denominator)?;
        Ok((
            u64::try_from(quotient).map_err(|_| Error::ArithmeticOverflow)?,
            remainder,
        ))
    }

    /// Exact overflow-free rational ordering by continued fractions.
    fn compare(self, other: Self) -> Result<core::cmp::Ordering> {
        let mut left_numerator = self.numerator;
        let mut left_denominator = self.denominator;
        let mut right_numerator = other.numerator;
        let mut right_denominator = other.denominator;
        let mut reversed = false;
        loop {
            let left_integer = left_numerator / left_denominator;
            let right_integer = right_numerator / right_denominator;
            if left_integer != right_integer {
                let ordering = left_integer.cmp(&right_integer);
                return Ok(if reversed {
                    ordering.reverse()
                } else {
                    ordering
                });
            }
            let left_remainder = left_numerator % left_denominator;
            let right_remainder = right_numerator % right_denominator;
            match (left_remainder == 0, right_remainder == 0) {
                (true, true) => return Ok(core::cmp::Ordering::Equal),
                (true, false) => {
                    let ordering = core::cmp::Ordering::Less;
                    return Ok(if reversed {
                        ordering.reverse()
                    } else {
                        ordering
                    });
                }
                (false, true) => {
                    let ordering = core::cmp::Ordering::Greater;
                    return Ok(if reversed {
                        ordering.reverse()
                    } else {
                        ordering
                    });
                }
                (false, false) => {
                    left_numerator = left_denominator;
                    left_denominator = left_remainder;
                    right_numerator = right_denominator;
                    right_denominator = right_remainder;
                    reversed = !reversed;
                }
            }
        }
    }
}

/// Exact Piegl-Tiller `BasisFuns` recurrence over one known span.
///
/// Endpoint multiplicity, all clamped end panes, and the cases where left and
/// right edge effects overlap are handled by the expanded knot array itself;
/// there are no interior-formula substitutions.
#[cfg(test)]
fn basis_functions(
    span: usize,
    degree: usize,
    value: u128,
    knots: &[u128; MAX_EXPANDED_KNOTS],
) -> Result<[Fraction; MAX_LOCAL_BASIS]> {
    let mut basis = [Fraction::ZERO; MAX_LOCAL_BASIS];
    let mut left = [0_u128; MAX_LOCAL_BASIS];
    let mut right = [0_u128; MAX_LOCAL_BASIS];
    basis[0] = Fraction::ONE;
    let mut column = 1_usize;
    while column <= degree {
        left[column] = value
            .checked_sub(knots[span + 1 - column])
            .ok_or(Error::ArithmeticOverflow)?;
        right[column] = knots[span + column]
            .checked_sub(value)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut saved = Fraction::ZERO;
        let mut row = 0_usize;
        while row < column {
            let divisor = right[row + 1]
                .checked_add(left[column - row])
                .ok_or(Error::ArithmeticOverflow)?;
            let temporary = basis[row].divide_integer(divisor)?;
            basis[row] = saved.add(temporary.multiply_integer(right[row + 1])?)?;
            saved = temporary.multiply_integer(left[column - row])?;
            row += 1;
        }
        basis[column] = saved;
        column += 1;
    }
    Ok(basis)
}

#[cfg(test)]
fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(degree: u8, knots_in: &[u128], denominator: u64) -> BasisSpec {
        let mut knots = [0_u128; MAX_KNOTS];
        knots[..knots_in.len()].copy_from_slice(knots_in);
        BasisSpec {
            outcome_count: if degree == 0 {
                knots_in.len() as u8 + 1
            } else {
                knots_in.len() as u8 - 1 + degree
            },
            degree,
            knot_count: knots_in.len() as u8,
            uniform_log2_spacing: if degree >= 2 { 2 } else { UNIFORM_SPACING_NONE },
            denominator,
            domain_max: 1_000,
            edge_policy: EdgePolicy::Clamp,
            knots,
        }
    }

    fn fraction_oracle(basis: &BasisSpec, value: u128) -> Result<WeightVector> {
        basis.validate()?;
        if basis.degree < 2 {
            return basis.evaluate(value);
        }
        let handled = basis.handle_edge(value)?;
        let last = basis.knots[usize::from(basis.knot_count) - 1];
        if handled <= basis.knots[0] {
            return WeightVector::one_hot(basis.outcome_count, basis.denominator, 0);
        }
        if handled >= last {
            return WeightVector::one_hot(
                basis.outcome_count,
                basis.denominator,
                usize::from(basis.outcome_count) - 1,
            );
        }
        let pane = basis.locate_pane(handled)?;
        basis.evaluate_smooth_fraction_oracle(handled, pane)
    }

    fn assert_fixed_matches_fraction(basis: &BasisSpec, value: u128) {
        let expected = fraction_oracle(basis, value);
        assert_eq!(basis.evaluate(value), expected);
        assert_eq!(
            basis
                .validated()
                .and_then(|valid| valid.evaluate_point(value)),
            expected
        );
    }

    #[test]
    fn fixed_uniform_path_matches_fraction_oracle_on_every_smooth_pane() {
        let denominators = [1_u64, 2, 3, 7, 257, u16::MAX.into()];
        let mut degree = 2_u8;
        while degree <= 3 {
            let maximum_knots = MAX_OUTCOMES + 1 - usize::from(degree);
            let mut shift = 0_u8;
            while shift <= 8 {
                let spacing = 1_u128 << shift;
                let mut knot_count = 2_usize;
                while knot_count <= maximum_knots {
                    let first = 13_u128;
                    let mut knots = [0_u128; MAX_KNOTS];
                    let mut knot = 0_usize;
                    while knot < knot_count {
                        knots[knot] = first + (knot as u128 * spacing);
                        knot += 1;
                    }
                    for denominator in denominators {
                        let basis = BasisSpec {
                            outcome_count: (knot_count - 1 + usize::from(degree)) as u8,
                            degree,
                            knot_count: knot_count as u8,
                            uniform_log2_spacing: shift,
                            denominator,
                            domain_max: knots[knot_count - 1] + 1,
                            edge_policy: EdgePolicy::Clamp,
                            knots,
                        };
                        // Both clamped outside edges and every pane's near-edge,
                        // interior, and exact right-boundary coordinates.
                        assert_fixed_matches_fraction(&basis, first - 1);
                        assert_fixed_matches_fraction(&basis, knots[knot_count - 1] + 1);
                        let offsets = [
                            0,
                            1.min(spacing),
                            spacing / 4,
                            spacing / 3,
                            spacing / 2,
                            spacing.saturating_sub(1),
                            spacing,
                        ];
                        let mut pane = 0_usize;
                        while pane + 1 < knot_count {
                            for offset in offsets {
                                assert_fixed_matches_fraction(
                                    &basis,
                                    knots[pane].checked_add(offset).unwrap(),
                                );
                            }
                            pane += 1;
                        }
                    }
                    knot_count += 1;
                }
                shift += 1;
            }
            degree += 1;
        }
    }

    #[test]
    fn fixed_uniform_path_matches_fraction_oracle_near_admission_bounds() {
        for (degree, shift) in [(2_u8, 62_u8), (3, 41)] {
            let spacing = 1_u128 << shift;
            let basis = BasisSpec {
                uniform_log2_spacing: shift,
                domain_max: u128::MAX,
                ..spec(degree, &[0, spacing], 1)
            };
            for value in [0, 1, spacing / 3, spacing / 2, spacing - 1, spacing] {
                assert_fixed_matches_fraction(&basis, value);
            }

            let translated_low = u128::MAX - spacing;
            let translated = BasisSpec {
                uniform_log2_spacing: shift,
                domain_max: u128::MAX,
                ..spec(degree, &[translated_low, u128::MAX], 1)
            };
            for offset in [0, 1, spacing / 3, spacing / 2, spacing - 1, spacing] {
                assert_fixed_matches_fraction(&translated, translated_low + offset);
            }
        }

        let spacing = 1_u128 << 20;
        let scaled = BasisSpec {
            uniform_log2_spacing: 20,
            denominator: u64::MAX,
            domain_max: u128::MAX,
            ..spec(3, &[0, spacing], 1)
        };
        for value in [1, spacing / 3, spacing / 2, spacing - 1] {
            assert_fixed_matches_fraction(&scaled, value);
        }
    }

    #[test]
    fn validated_capability_preserves_refusal_classes() {
        let mut refusing = spec(3, &[4, 8, 12], 257);
        refusing.edge_policy = EdgePolicy::Refuse;
        let validated = refusing.validated().unwrap();
        assert_eq!(validated.evaluate_point(3), Err(Error::ValueOutOfRange));
        assert_eq!(validated.evaluate_point(13), Err(Error::ValueOutOfRange));

        let mut malformed = refusing;
        malformed.uniform_log2_spacing = 1;
        assert_eq!(malformed.validated(), Err(Error::UniformSpacingMismatch));
        malformed = refusing;
        malformed.knots[MAX_KNOTS - 1] = 1;
        assert_eq!(malformed.validated(), Err(Error::NonCanonicalPadding));
        malformed = refusing;
        malformed.uniform_log2_spacing = UNIFORM_SPACING_NONE;
        assert_eq!(malformed.validated(), Err(Error::UniformSpacingRequired));
    }

    #[test]
    fn degree_zero_is_the_existing_closed_top_partition() {
        let basis = spec(0, &[10, 20, 30], 7);
        assert_eq!(&basis.evaluate(0).unwrap().weights[..4], &[7, 0, 0, 0]);
        assert_eq!(&basis.evaluate(9).unwrap().weights[..4], &[7, 0, 0, 0]);
        assert_eq!(&basis.evaluate(10).unwrap().weights[..4], &[0, 7, 0, 0]);
        assert_eq!(&basis.evaluate(30).unwrap().weights[..4], &[0, 0, 0, 7]);
        assert_eq!(
            &basis.evaluate(u128::MAX).unwrap().weights[..4],
            &[0, 0, 0, 7]
        );
    }

    #[test]
    fn degree_one_uses_nearest_largest_remainder_and_exact_shifts() {
        let mut nonuniform = spec(1, &[0, 8], 7);
        nonuniform.uniform_log2_spacing = UNIFORM_SPACING_NONE;
        assert_eq!(&nonuniform.evaluate(1).unwrap().weights[..2], &[6, 1]);
        assert_eq!(&nonuniform.evaluate(4).unwrap().weights[..2], &[4, 3]);

        let mut exact = spec(1, &[0, 8], 8);
        exact.uniform_log2_spacing = 3;
        let mut coordinate = 0_u128;
        while coordinate <= 8 {
            let vector = exact.evaluate(coordinate).unwrap();
            assert_eq!(vector.weights[0], 8 - coordinate as u64);
            assert_eq!(vector.weights[1], coordinate as u64);
            coordinate += 1;
        }
    }

    #[test]
    fn clamped_quadratic_and_cubic_edges_are_bezier_on_one_pane() {
        let quadratic = spec(2, &[0, 4], 16);
        assert_eq!(&quadratic.evaluate(0).unwrap().weights[..3], &[16, 0, 0]);
        assert_eq!(&quadratic.evaluate(2).unwrap().weights[..3], &[4, 8, 4]);
        assert_eq!(&quadratic.evaluate(4).unwrap().weights[..3], &[0, 0, 16]);

        let cubic = spec(3, &[0, 4], 64);
        assert_eq!(&cubic.evaluate(0).unwrap().weights[..4], &[64, 0, 0, 0]);
        assert_eq!(&cubic.evaluate(2).unwrap().weights[..4], &[8, 24, 24, 8]);
        assert_eq!(&cubic.evaluate(4).unwrap().weights[..4], &[0, 0, 0, 64]);
    }

    #[test]
    fn every_pane_including_edges_has_bounded_support_and_exact_sum() {
        let mut degree = 2_u8;
        while degree <= 3 {
            let basis = spec(degree, &[3, 7, 11, 15, 19], 257);
            let mut value = 0_u128;
            while value <= 22 {
                let vector = basis.evaluate(value).unwrap();
                vector.validate().unwrap();
                let support = vector.weights.iter().filter(|&&weight| weight != 0).count();
                assert!(support <= usize::from(degree) + 1);
                value += 1;
            }
            degree += 1;
        }
    }

    #[test]
    fn refusing_edges_and_malformed_specs_fail_closed() {
        let mut basis = spec(2, &[4, 8, 12], 16);
        basis.edge_policy = EdgePolicy::Refuse;
        assert_eq!(basis.evaluate(3), Err(Error::ValueOutOfRange));
        assert_eq!(basis.evaluate(13), Err(Error::ValueOutOfRange));

        let mut lying = basis;
        lying.uniform_log2_spacing = 1;
        assert_eq!(lying.validate(), Err(Error::UniformSpacingMismatch));
        let mut padded = basis;
        padded.knots[15] = 1;
        assert_eq!(padded.validate(), Err(Error::NonCanonicalPadding));
        let mut nonuniform = basis;
        nonuniform.uniform_log2_spacing = UNIFORM_SPACING_NONE;
        assert_eq!(nonuniform.validate(), Err(Error::UniformSpacingRequired));
    }

    #[test]
    fn independent_floor_mutant_breaks_partition_unity() {
        let basis = spec(2, &[0, 4], 7);
        let expanded = basis.expanded_knots().unwrap();
        let exact = basis_functions(2, 2, 2, &expanded).unwrap();
        let mutant_sum = exact[0].scaled_parts(7).unwrap().0
            + exact[1].scaled_parts(7).unwrap().0
            + exact[2].scaled_parts(7).unwrap().0;
        assert_eq!(mutant_sum, 5);
        assert_eq!(basis.evaluate(2).unwrap().weights[..3], [2, 3, 2]);
    }

    #[test]
    fn largest_remainder_ties_choose_the_lowest_outcome() {
        let basis = spec(1, &[0, 4], 1);
        assert_eq!(&basis.evaluate(2).unwrap().weights[..2], &[1, 0]);

        let quadratic = spec(2, &[0, 4], 2);
        // Exact scaled values are (1/2, 1, 1/2). The one residual atom is
        // tied between the two edge outcomes and goes to the lower index.
        assert_eq!(&quadratic.evaluate(2).unwrap().weights[..3], &[1, 1, 0]);
    }

    #[test]
    fn largest_remainder_commutes_with_reflection_when_remainders_are_distinct() {
        let basis = spec(2, &[0, 4], 7);
        let left = basis.evaluate(1).unwrap();
        let right = basis.evaluate(3).unwrap();
        assert_eq!(&left.weights[..3], &[4, 3, 0]);
        assert_eq!(&right.weights[..3], &[0, 3, 4]);
    }

    #[test]
    fn internal_pane_boundaries_use_the_continuous_right_span() {
        let quadratic = spec(2, &[0, 4, 8], 8);
        assert_eq!(&quadratic.evaluate(4).unwrap().weights[..4], &[0, 4, 4, 0]);
        let cubic = spec(3, &[0, 4, 8], 8);
        assert_eq!(&cubic.evaluate(4).unwrap().weights[..5], &[0, 2, 4, 2, 0]);
    }

    #[test]
    fn freeze_bound_accepts_last_safe_pow2_and_refuses_the_next() {
        let accepted_cubic = BasisSpec {
            uniform_log2_spacing: 41,
            domain_max: u128::MAX,
            ..spec(3, &[0, 1_u128 << 41], 1)
        };
        accepted_cubic.validate().unwrap();
        accepted_cubic.evaluate(1_u128 << 40).unwrap();
        let rejected_cubic = BasisSpec {
            uniform_log2_spacing: 42,
            domain_max: u128::MAX,
            ..spec(3, &[0, 1_u128 << 42], 1)
        };
        assert_eq!(rejected_cubic.validate(), Err(Error::ArithmeticBound));

        let accepted_quadratic = BasisSpec {
            uniform_log2_spacing: 62,
            domain_max: u128::MAX,
            ..spec(2, &[0, 1_u128 << 62], 1)
        };
        accepted_quadratic.validate().unwrap();
        accepted_quadratic.evaluate(1_u128 << 61).unwrap();
        let rejected_quadratic = BasisSpec {
            uniform_log2_spacing: 63,
            domain_max: u128::MAX,
            ..spec(2, &[0, 1_u128 << 63], 1)
        };
        assert_eq!(rejected_quadratic.validate(), Err(Error::ArithmeticBound));

        let accepted_scaled = BasisSpec {
            uniform_log2_spacing: 20,
            denominator: u64::MAX,
            domain_max: u128::MAX,
            ..spec(3, &[0, 1_u128 << 20], 1)
        };
        accepted_scaled.validate().unwrap();
        accepted_scaled.evaluate(1_u128 << 19).unwrap();
        let rejected_scaled = BasisSpec {
            uniform_log2_spacing: 21,
            denominator: u64::MAX,
            domain_max: u128::MAX,
            ..spec(3, &[0, 1_u128 << 21], 1)
        };
        assert_eq!(rejected_scaled.validate(), Err(Error::ArithmeticBound));
    }

    #[test]
    fn interior_cardinal_mutant_does_not_describe_clamped_edge_panes() {
        let basis = spec(2, &[0, 4, 8, 12], 64);
        let actual = basis.evaluate(2).unwrap();
        // The interior cardinal quadratic formula at u=h/2 is (1, 6, 1)/8;
        // the first open-clamped pane is different and must be (1, 5, 2)/4.
        assert_eq!(&actual.weights[..3], &[16, 40, 8]);
        assert_ne!(&actual.weights[..3], &[8, 48, 8]);
    }

    #[test]
    fn open_top_mutant_is_killed_by_closed_top_semantics() {
        let basis = spec(3, &[0, 4, 8], 17);
        let top = basis.evaluate(8).unwrap();
        assert_eq!(top.weights[usize::from(basis.outcome_count) - 1], 17);
        assert_eq!(top.weights.iter().copied().sum::<u64>(), 17);
    }
}
