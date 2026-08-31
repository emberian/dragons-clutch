//! Integer Cox-de-Boor for the degree-2-to-3 basis family, at the live wire's
//! widths.
//!
//! # What this module is, and what it deliberately is not
//!
//! This is the port `BASIS_ABI_UNIFICATION_V1` section 5 calls for: the
//! algorithm from `dclutch-liability-basis-v2-kernel`'s `spline.rs`, carried
//! into the crate that owns the live wire, widened from the kernel's
//! `i64`/`u32` to the live record's `i128`/`u64`, and checked against the
//! kernel as a differential reference under `O-005`.
//!
//! **No route reaches this module.** [`crate::spline_admission_v3::SPLINE_EVALUATOR_RELEASED_V3`]
//! is still `false` and [`crate::runtime_v3::BasisKindV3::decode`] still
//! refuses tag 3, so no byte string this codec accepts changed when the port
//! landed. What the port buys is that the algorithm now exists where the wire
//! is owned, under the corpus, with its agreement against the kernel measured
//! rather than assumed.
//!
//! # The weights are ported; the rounding rule is not yet ruled
//!
//! The port stops at the exact rational weights on purpose, and this is the
//! one substantive thing the implementing lane found that the design could not
//! have known.
//!
//! The ruling says to adopt "the live wire's rounding rule". Measured, that
//! rule is: floor each primary term independently, then hand the *last* claim
//! `Q - sum(primary)` (`runtime_v3.rs`, `evaluate_rational`). It is
//! well-defined there because the live graded family **structurally reserves**
//! that last claim — `primary_count = basis_width - 1`, and a term whose
//! `claim_index` reaches it is refused, so the complement claim never carries a
//! curve of its own.
//!
//! **A spline has no such claim.** Every one of its `K = knot_count - degree - 1`
//! claims carries a structural de Boor weight, and the claims outside the local
//! support carry an *exact zero* that
//! `SplineProfile.evaluate_zero_outside_support` is stated about. Transliterating
//! the live rule would hand the rounding residue to whichever claim happens to
//! be last — frequently one whose exact weight is zero. That is not the live
//! rule ported; it is a different rule that happens to compile.
//!
//! So both candidate boundaries are implemented here, neither is blessed, and
//! [`apportionment_divergence_v3`] measures the gap between them. Which one the
//! family ships is a money decision and a price-gate soundness decision — the
//! gate's hull identity recomputes every atom through the production evaluator
//! — and it belongs to the commit that first *accepts* a kind-3 body, not to
//! this one.
//!
//! # Overflow
//!
//! As in the kernel, overflow has no Lean counterpart: the specification
//! quantifies over unbounded `Int`. This module evaluates inside an
//! `i128`/`u128` envelope and refuses [`Error::ArithmeticOverflow`] the moment
//! a checked operation would leave it. The envelope is genuinely narrower than
//! the live record's declared types permit — a knot near `i128::MAX` scaled by
//! a `u64` coordinate denominator does not fit — so this is a fail-closed
//! refusal, not a proof of totality. Widening the accumulation to the crate's
//! `SignedU256` is named in the report as remaining work rather than quietly
//! assumed away.

use crate::runtime_v3::{
    BASIS_SPLINE_MAXIMUM_DEGREE_V3, BASIS_SPLINE_MINIMUM_DEGREE_V3, Error, Result,
};

/// Claims one coordinate can be locally supported on: `degree + 1`, at the
/// profile's maximum degree.
pub const SPLINE_MAX_SUPPORT_V3: usize = (BASIS_SPLINE_MAXIMUM_DEGREE_V3 as usize) + 1;

/// Weights one Cox-de-Boor level carries at most: `degree` of them.
const LEVEL_CAPACITY: usize = SPLINE_MAX_SUPPORT_V3 - 1;

/// The exact rational B-spline weights of one coordinate.
///
/// The weights are integer numerators over one common positive
/// [`denominator`](Self::denominator). No rounding decision has been taken at
/// this point; only the apportionment functions take one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplineWeightsV3 {
    /// Local de Boor numerators, `local_len` of them from index zero.
    pub local: [u128; SPLINE_MAX_SUPPORT_V3],
    /// Valid entries of `local`, always `degree + 1`.
    pub local_len: usize,
    /// First claim the local support covers: `span - degree`.
    pub offset: usize,
    /// Common positive denominator every entry of `local` is a numerator over.
    pub denominator: u128,
    /// Runtime claim width the weights scatter into.
    pub width: usize,
    /// The located non-degenerate knot span.
    pub span: usize,
}

impl SplineWeightsV3 {
    /// The exact weight numerator of one claim.
    ///
    /// Claims outside the local support carry an exact zero rather than a
    /// rounded one.
    pub fn numerator_at(&self, claim: usize) -> u128 {
        match claim.checked_sub(self.offset) {
            Some(local) if local < self.local_len => read(&self.local, local).unwrap_or(0),
            _ => 0,
        }
    }
}

/// Evaluate the exact rational B-spline weights at one coordinate.
///
/// `knots` are the record's active knot numerators over `knot_denominator`;
/// the coordinate is `coordinate_numerator / coordinate_denominator`. Both are
/// carried onto the single common denominator `knot_denominator *
/// coordinate_denominator`, so the located span, the clamp and every de Boor
/// weight are exact integer comparisons rather than rational ones.
///
/// This is the kernel's `evaluate_spline_basis_v2` at the live wire's widths.
/// The structural refusals are [`Error::SplineDegreeOutOfProfile`] for a degree
/// outside the family, [`Error::SplineWidthDerivationMismatch`] when the knot
/// vector does not derive the declared width, and
/// [`Error::SplineDegenerateSpan`] when no non-degenerate span exists or the
/// located span leaves the domain.
///
/// Repeated interior knots are admitted here — a knot of multiplicity `r`
/// collapses `r - 1` spans and those are skipped rather than refused, which is
/// what makes interior multiplicity, and so a corner inside an otherwise
/// smooth basis, expressible at all. That is a property of this evaluator
/// only; the two shipping kinds keep their strict knot ordering, which is
/// [`crate::runtime_v3`]'s business and is unchanged.
pub fn evaluate_spline_weights_v3(
    knots: &[i128],
    knot_denominator: u64,
    coordinate_numerator: i128,
    coordinate_denominator: u64,
    degree: u8,
    width: u32,
) -> Result<SplineWeightsV3> {
    if !(BASIS_SPLINE_MINIMUM_DEGREE_V3..=BASIS_SPLINE_MAXIMUM_DEGREE_V3).contains(&degree) {
        return Err(Error::SplineDegreeOutOfProfile);
    }
    if knot_denominator == 0 || coordinate_denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    let degree = usize::from(degree);
    let width = usize::try_from(width).map_err(|_| Error::InvalidCount)?;
    // `width = knot_count - degree - 1`, the standard B-spline count and the
    // sole binding between the degree and the knot vector.
    let derived = knots
        .len()
        .checked_sub(degree)
        .and_then(|value| value.checked_sub(1))
        .filter(|value| *value > 0)
        .ok_or(Error::SplineWidthDerivationMismatch)?;
    if derived != width {
        return Err(Error::SplineWidthDerivationMismatch);
    }

    let scaled = |index: usize| -> Result<i128> {
        match knots.get(index) {
            Some(knot) => knot
                .checked_mul(i128::from(coordinate_denominator))
                .ok_or(Error::ArithmeticOverflow),
            // Total, matching Lean's `knotAt`. Every index this evaluator
            // forms is in range for a record whose width derivation checked
            // out above; the branch keeps the function total rather than
            // panicking if one ever is not.
            None => Ok(0),
        }
    };
    let coordinate = coordinate_numerator
        .checked_mul(i128::from(knot_denominator))
        .ok_or(Error::ArithmeticOverflow)?;
    // Below the domain the first claims pay their full weight, above it the
    // last ones do, rather than the coordinate falling off a half-open span.
    let clamped = scaled(degree)?.max(coordinate.min(scaled(width)?));
    let span = locate_span(&scaled, degree, width, clamped)?;
    let offset = span
        .checked_sub(degree)
        .ok_or(Error::SplineDegenerateSpan)?;

    let mut values = [0_u128; SPLINE_MAX_SUPPORT_V3];
    write(&mut values, 0, 1)?;
    let mut len = 1_usize;
    let mut denominator = 1_u128;
    for level in 1..=degree {
        let (numerators, denominators) = level_weights(&scaled, clamped, span, level)?;
        let suffix = suffix_products(&denominators, level)?;
        // One degree-raising step is a convex redistribution: under the weight
        // `p/q` each value sends `(q-p)*v` left and `p*v` right, and every
        // value already placed to the right is scaled by `q` so the level stays
        // over one common denominator. That is structurally sum-preserving,
        // which is why the partition of unity needs no reindexing argument.
        let mut raised = [0_u128; SPLINE_MAX_SUPPORT_V3];
        for lower in (0..level).rev() {
            let numerator = read(&numerators, lower)?;
            let divisor = read(&denominators, lower)?;
            let right = step(lower)?;
            let carried = read(&suffix, right)?;
            let value = read(&values, lower)?;
            for higher in step(right)?..=level {
                let held = read(&raised, higher)?;
                write(&mut raised, higher, checked_mul(divisor, held)?)?;
            }
            let held = read(&raised, right)?;
            let sent_right = checked_mul(checked_mul(numerator, value)?, carried)?
                .checked_add(checked_mul(divisor, held)?)
                .ok_or(Error::ArithmeticOverflow)?;
            write(&mut raised, right, sent_right)?;
            // `numerator <= divisor` is structural: the level clamps it.
            let complement = divisor
                .checked_sub(numerator)
                .ok_or(Error::ArithmeticOverflow)?;
            let sent_left = checked_mul(checked_mul(complement, value)?, carried)?;
            write(&mut raised, lower, sent_left)?;
        }
        values = raised;
        len = step(level)?;
        denominator = checked_mul(denominator, read(&suffix, 0)?)?;
    }

    if offset.checked_add(len).is_none_or(|end| end > width) {
        return Err(Error::SplineDegenerateSpan);
    }
    Ok(SplineWeightsV3 {
        local: values,
        local_len: len,
        offset,
        denominator,
        width,
        span,
    })
}

/// Apportion `payout_scale` atoms by cumulative-floor telescoping.
///
/// **The kernel's boundary**, ported unchanged: each claim receives the
/// difference between two consecutive floors of the *running* weight sum.
/// Because the weights sum to the denominator the final floor is exactly the
/// scale, so the payouts telescope to it with no residue to place — and every
/// claim lands within one atom of its exact rational share.
///
/// It also keeps the exact zero outside the local support, because a claim
/// whose weight is zero does not advance the running sum and so receives the
/// difference of two equal floors.
pub fn apportion_cumulative_v3(
    weights: &SplineWeightsV3,
    payout_scale: u64,
    output: &mut [u64],
) -> Result<()> {
    preflight(weights, payout_scale, output)?;
    let mut running = 0_u128;
    let mut carried = 0_u128;
    for claim in 0..weights.width {
        running = running
            .checked_add(weights.numerator_at(claim))
            .ok_or(Error::ArithmeticOverflow)?;
        let boundary = u128::from(payout_scale)
            .checked_mul(running)
            .ok_or(Error::ArithmeticOverflow)?
            .checked_div(weights.denominator)
            .ok_or(Error::ZeroDenominator)?;
        let payout = boundary.checked_sub(carried).ok_or(Error::NonPartition)?;
        let slot = output.get_mut(claim).ok_or(Error::InvalidLength)?;
        *slot = u64::try_from(payout).map_err(|_| Error::ArithmeticOverflow)?;
        carried = boundary;
    }
    // Fail-closed rather than reachable: the de Boor step derives the partition
    // of unity structurally, so a triangle reaching here sums to its own
    // denominator.
    if running != weights.denominator {
        return Err(Error::NonPartition);
    }
    Ok(())
}

/// Apportion `payout_scale` atoms by per-claim floor with an exact complement.
///
/// **The live family's discipline, transliterated as closely as a spline
/// admits.** Each claim's exact share is floored independently; the residue
/// `Q - sum(floors)` goes to the *last locally supported* claim rather than to
/// the last claim of the width.
///
/// That deviation from `runtime_v3`'s literal `output.last_mut()` is forced,
/// and it is the reason this module does not simply declare the live rule
/// ported. The live rule's absorber is a structurally term-free complement
/// claim; a spline has none, and its trailing claims routinely carry an exact
/// zero. Sending residue to a zero-weight claim would pay a claim the basis
/// says is unsupported. Sending it to the last supported claim keeps the
/// support exact and keeps the partition exact, at the cost of letting that one
/// claim sit up to `degree` atoms above its exact share — which is precisely
/// what [`apportion_cumulative_v3`] does not do, and precisely what the price
/// gate's hull identity would have to be re-checked against.
pub fn apportion_floor_complement_v3(
    weights: &SplineWeightsV3,
    payout_scale: u64,
    output: &mut [u64],
) -> Result<()> {
    preflight(weights, payout_scale, output)?;
    let mut total = 0_u64;
    for claim in 0..weights.width {
        let floored = u128::from(payout_scale)
            .checked_mul(weights.numerator_at(claim))
            .ok_or(Error::ArithmeticOverflow)?
            .checked_div(weights.denominator)
            .ok_or(Error::ZeroDenominator)?;
        let payout = u64::try_from(floored).map_err(|_| Error::ArithmeticOverflow)?;
        let slot = output.get_mut(claim).ok_or(Error::InvalidLength)?;
        *slot = payout;
        total = total.checked_add(payout).ok_or(Error::ArithmeticOverflow)?;
    }
    let residue = payout_scale.checked_sub(total).ok_or(Error::NonPartition)?;
    // The last claim the local support covers. `local_len >= 1` and the
    // support was bounds-checked into the width by the evaluator, so this
    // index is in range.
    let absorber = weights
        .offset
        .checked_add(weights.local_len)
        .and_then(|end| end.checked_sub(1))
        .ok_or(Error::ArithmeticOverflow)?;
    let slot = output.get_mut(absorber).ok_or(Error::InvalidLength)?;
    *slot = slot.checked_add(residue).ok_or(Error::ArithmeticOverflow)?;
    Ok(())
}

/// The largest per-claim gap between the two candidate boundaries, in atoms.
///
/// Zero means the two rules agree exactly on this coordinate. This exists so
/// the divergence between the kernel's boundary and the live family's
/// discipline is a measured number in a test rather than a claim in a comment,
/// and so the commit that rules on the rounding rule can see what it is
/// choosing between.
pub fn apportionment_divergence_v3(
    weights: &SplineWeightsV3,
    payout_scale: u64,
    cumulative: &mut [u64],
    floor_complement: &mut [u64],
) -> Result<u64> {
    apportion_cumulative_v3(weights, payout_scale, cumulative)?;
    apportion_floor_complement_v3(weights, payout_scale, floor_complement)?;
    let mut worst = 0_u64;
    for claim in 0..weights.width {
        let left = *cumulative.get(claim).ok_or(Error::InvalidLength)?;
        let right = *floor_complement.get(claim).ok_or(Error::InvalidLength)?;
        let gap = left.abs_diff(right);
        if gap > worst {
            worst = gap;
        }
    }
    Ok(worst)
}

fn preflight(weights: &SplineWeightsV3, payout_scale: u64, output: &[u64]) -> Result<()> {
    if payout_scale == 0 {
        return Err(Error::ZeroScale);
    }
    if weights.denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    if weights.width == 0 {
        return Err(Error::InvalidCount);
    }
    if output.len() != weights.width {
        return Err(Error::InvalidLength);
    }
    Ok(())
}

/// The non-degenerate span carrying the clamped coordinate.
///
/// A knot of multiplicity `r` collapses `r - 1` spans; those are skipped
/// rather than refused, which is what admits interior multiplicity.
fn locate_span(
    scaled: &impl Fn(usize) -> Result<i128>,
    degree: usize,
    width: usize,
    clamped: i128,
) -> Result<usize> {
    let mut first = None;
    let mut located = None;
    for span in degree..width {
        let low = scaled(span)?;
        let high = scaled(span.checked_add(1).ok_or(Error::SplineDegenerateSpan)?)?;
        if low < high {
            if first.is_none() {
                first = Some(span);
            }
            if low <= clamped {
                located = Some(span);
            }
        }
    }
    // The top of the domain is closed, so the final coordinate lands in the
    // final span; a coordinate below every candidate lands in the first one.
    let span = located.or(first).ok_or(Error::SplineDegenerateSpan)?;
    if span < degree || span >= width {
        return Err(Error::SplineDegenerateSpan);
    }
    Ok(span)
}

/// The `level` Cox-de-Boor weights of one triangle level, as numerator and
/// denominator pairs at knot indices `span + 1 - level` through `span`.
///
/// The numerator is clamped to the denominator so no weight exceeds one; on a
/// correctly located span the clamp is inert, and keeping it makes
/// `numerator <= denominator` structural rather than an assumed lemma.
fn level_weights(
    scaled: &impl Fn(usize) -> Result<i128>,
    clamped: i128,
    span: usize,
    level: usize,
) -> Result<([u128; LEVEL_CAPACITY], [u128; LEVEL_CAPACITY])> {
    let mut numerators = [0_u128; LEVEL_CAPACITY];
    let mut denominators = [0_u128; LEVEL_CAPACITY];
    for offset in 0..level.min(LEVEL_CAPACITY) {
        // `level <= degree <= span`, so this never underflows.
        let index = span
            .checked_add(1)
            .and_then(|shifted| shifted.checked_add(offset))
            .and_then(|shifted| shifted.checked_sub(level))
            .ok_or(Error::SplineDegenerateSpan)?;
        let base = scaled(index)?;
        let support = nonnegative(
            scaled(
                index
                    .checked_add(level)
                    .ok_or(Error::SplineDegenerateSpan)?,
            )?
            .checked_sub(base)
            .ok_or(Error::ArithmeticOverflow)?,
        )?;
        if support == 0 {
            return Err(Error::SplineDegenerateSpan);
        }
        let elapsed = nonnegative(clamped.checked_sub(base).ok_or(Error::ArithmeticOverflow)?)?;
        write(&mut numerators, offset, elapsed.min(support))?;
        write(&mut denominators, offset, support)?;
    }
    Ok((numerators, denominators))
}

/// Suffix products of one level's denominators, `suffix[level] == 1`.
///
/// `suffix[0]` is the factor the whole triangle's common denominator picks up
/// from this level.
fn suffix_products(
    denominators: &[u128; LEVEL_CAPACITY],
    level: usize,
) -> Result<[u128; SPLINE_MAX_SUPPORT_V3]> {
    let mut suffix = [1_u128; SPLINE_MAX_SUPPORT_V3];
    for lower in (0..level).rev() {
        let above = read(&suffix, step(lower)?)?;
        let product = checked_mul(read(denominators, lower)?, above)?;
        write(&mut suffix, lower, product)?;
    }
    Ok(suffix)
}

/// Clamp one exact signed difference to a nonnegative magnitude. Lean's
/// `Int.toNat`, which is `max 0` rather than a truncation.
fn nonnegative(value: i128) -> Result<u128> {
    if value <= 0 {
        return Ok(0);
    }
    u128::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

fn checked_mul(left: u128, right: u128) -> Result<u128> {
    left.checked_mul(right).ok_or(Error::ArithmeticOverflow)
}

fn step(index: usize) -> Result<usize> {
    index.checked_add(1).ok_or(Error::ArithmeticOverflow)
}

/// Total read of one fixed-size triangle buffer. Every call site is
/// structurally in range; an out-of-range read is a fail-closed refusal rather
/// than a panic, so this module cannot abort.
fn read(values: &[u128], index: usize) -> Result<u128> {
    values.get(index).copied().ok_or(Error::ArithmeticOverflow)
}

/// Total write into one fixed-size triangle buffer. See [`read`].
fn write(values: &mut [u128], index: usize, value: u128) -> Result<()> {
    *values.get_mut(index).ok_or(Error::ArithmeticOverflow)? = value;
    Ok(())
}
