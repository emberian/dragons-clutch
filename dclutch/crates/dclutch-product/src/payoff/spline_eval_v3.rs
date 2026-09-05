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
//! **No route reaches this module.** [`crate::payoff::spline_admission_v3::SPLINE_EVALUATOR_RELEASED_V3`]
//! is still `false` and [`crate::payoff::runtime_v3::BasisKindV3::decode`] still
//! refuses tag 3, so no byte string this codec accepts changed when the port
//! landed. What the port buys is that the algorithm now exists where the wire
//! is owned, under the corpus, with its agreement against the kernel measured
//! rather than assumed.
//!
//! # The rounding rule is cumulative-floor, and that is ruled
//!
//! The option-D directive said to adopt "the live wire's rounding rule". That
//! was under-determined for splines, and the reason is worth keeping.
//!
//! The live rule is: floor each primary term independently, then hand the
//! *last* claim `Q - sum(primary)` (`runtime_v3.rs`, `evaluate_rational`). It
//! is well-defined there because the live graded family **structurally
//! reserves** that last claim — `primary_count = basis_width - 1`, and a term
//! whose `claim_index` reaches it is refused, so the complement claim never
//! carries a curve of its own.
//!
//! **A spline reserves nothing.** Every one of its
//! `K = knot_count - degree - 1` claims carries a structural de Boor weight,
//! and the claims outside the local support carry an *exact zero* that
//! `SplineProfile.evaluate_zero_outside_support` is stated about.
//! Transliterating the live rule would hand the rounding residue to whichever
//! claim happens to be last — frequently one whose exact weight is zero. That
//! is not the live rule ported; it is a different rule that happens to compile,
//! and it pays a claim the basis says is unsupported.
//!
//! So both boundaries were implemented and measured, and the orchestrator ruled
//! on the measurement (WAVE `76e2ca3f`): **cumulative-floor is the spline
//! rounding rule**, binding on the commit that first accepts a kind-3 body.
//! Over eleven cases at both degrees, cumulative-floor kept every claim within
//! one atom of its exact share and preserved zero-outside-support;
//! floor-plus-complement did neither (2 of 11 diverged, worst 2 atoms).
//!
//! [`apportion_cumulative_v3`] is therefore the only apportionment this module
//! offers. The rejected implementation is **deleted rather than deprecated** —
//! a second rounding rule sitting next to the blessed one is a second author of
//! the money, and the two differ by real atoms. What survives the deletion is
//! the *grounds*: the two properties that decided the ruling are asserted
//! directly as properties of the blessed rule, so the ruling stays checkable
//! without the code it ruled against.
//!
//! # Overflow, and why a checked refusal was not enough
//!
//! As in the kernel, overflow has no Lean counterpart: the specification
//! quantifies over unbounded `Int`. This module carries the de Boor triangle in
//! a private fixed `[u64; 4]` integer and refuses [`Error::ArithmeticOverflow`]
//! the moment a checked operation would leave it. No wrong number is reachable,
//! and the wider accumulator is not a wire fact or a second arithmetic policy.
//!
//! **But a checked refusal at the wrong moment is still principal stranding.**
//! A basis admitted at founding whose triangle overflows at some coordinate
//! refuses at *settlement*, when the money is already in — the E5 class
//! wall 22 was about. Fail-closed arithmetically, a trap operationally. So the
//! envelope is closed twice over:
//!
//! 1. **The coordinate is saturated against the knot range before it is
//!    scaled.** The pre-scaling multiply at the coordinate is
//!    order-preserving-saturating rather than checked, and the clamp that
//!    immediately follows discards the magnitude. That is exact, not
//!    approximate: see [`evaluate_spline_weights_v3`]'s clamp for the argument.
//!    An oracle coordinate of any magnitude whatsoever can no longer trap.
//! 2. **The triangle's magnitude is bounded at admission, over the
//!    founding-fixed quantities only** — [`spline_arithmetic_envelope_v3`].
//!    The knots, the degree and the payout scale are all fixed when the Market
//!    is founded; the only settlement-time input left is the coordinate
//!    *denominator*, which the envelope quantifies over up to
//!    [`SPLINE_COORDINATE_DENOMINATOR_CEILING_V3`]. A basis that could overflow
//!    at any admissible coordinate is refused at founding, where refusing costs
//!    nobody their principal.
//!
//! What that leaves, stated rather than hidden: a coordinate denominator
//! *above* the published ceiling still refuses at settlement. Inside that
//! closed bound, the founding envelope and evaluator use the same 256-bit
//! capacity, including realistic degree-3 bases that the former `u128`
//! implementation could not admit.

use crate::payoff::{
    U256,
    runtime_v3::{BASIS_SPLINE_MAXIMUM_DEGREE_V3, BASIS_SPLINE_MINIMUM_DEGREE_V3, Error, Result},
};

/// Claims one coordinate can be locally supported on: `degree + 1`, at the
/// profile's maximum degree.
pub const SPLINE_MAX_SUPPORT_V3: usize = (BASIS_SPLINE_MAXIMUM_DEGREE_V3 as usize) + 1;

/// Weights one Cox-de-Boor level carries at most: `degree` of them.
const LEVEL_CAPACITY: usize = SPLINE_MAX_SUPPORT_V3 - 1;

/// Largest coordinate denominator a spline basis is admitted against.
///
/// The envelope in [`spline_arithmetic_envelope_v3`] quantifies over every
/// coordinate denominator up to this value, so a basis that passes admission
/// evaluates without overflow at all of them. It is `2^20`, chosen to cover the
/// six-decimal fixed-point denominators real price feeds publish (`1_000_000`
/// is `2^19.93`) with a little room, and it is a *published* boundary rather
/// than an incidental one: a coordinate denominator above it refuses, by name,
/// at [`Error::SplineCoordinateOutOfEnvelope`].
///
/// It is deliberately a constant and not a wire field. Putting it on the wire
/// would let a founder buy a wider coordinate domain by narrowing their knots,
/// which is a trade nobody should be able to make silently; and the record has
/// no room for it (the 50 reserved bytes are spent by the degree and the
/// certificate digest).
pub const SPLINE_COORDINATE_DENOMINATOR_CEILING_V3: u64 = 1 << 20;

/// A knot vector the arithmetic envelope can read by index.
///
/// The envelope is called from **founding, on chain**, where the knots live in
/// a borrowed Registry-owned account body and there is no allocator to collect
/// them into a slice. So it reads them by index from wherever they already are
/// rather than requiring a `&[i128]`: `ProductBasisV3` implements this directly
/// over its own record bytes, and `[i128]` implements it for callers that do
/// hold a slice.
///
/// [`evaluate_spline_weights_v3`] reads its knots the same way, for the same
/// reason: the price gate's hull identity recomputes every atom **through the
/// production evaluator**, from a record, at founding.
pub trait SplineKnotsV3 {
    /// The knot numerator at `index`, or `None` past the end.
    fn knot_at(&self, index: usize) -> Option<i128>;
    /// How many knots there are.
    fn knot_count(&self) -> usize;
}

impl SplineKnotsV3 for [i128] {
    fn knot_at(&self, index: usize) -> Option<i128> {
        self.get(index).copied()
    }

    fn knot_count(&self) -> usize {
        self.len()
    }
}

/// Whether this basis evaluates without overflow at *every* admissible
/// coordinate.
///
/// # Why this exists at all
///
/// Every arithmetic operation in this module is checked, so no wrong number is
/// reachable. That is not the same as safe. The quantities the de Boor triangle
/// multiplies are the knots, the degree and the payout scale — **all three
/// fixed when the Market is founded** — together with the coordinate, which
/// arrives at settlement from the resolution. Without this check a basis is
/// admitted at founding and discovers at settlement that its triangle does not
/// fit, which strands principal that is already in. Fail-closed arithmetically,
/// a trap operationally.
///
/// So this conjunct moves the discovery to founding, where a refusal costs
/// nobody anything, and quantifies away the one settlement-time input by
/// bounding it at [`SPLINE_COORDINATE_DENOMINATOR_CEILING_V3`].
///
/// # What it checks, and why that is exactly the right bound
///
/// For each span [`locate_span`] could select, it replays the triangle's
/// *denominator* recursion — the same knot differences [`level_weights`] takes
/// its supports from, carried onto the ceiling — and requires two things:
///
/// - every level's support is strictly positive, so no selectable span is
///   degenerate (a degenerate one refuses at settlement, which is the same
///   strand by another name);
/// - `payout_scale * denominator` fits the evaluator's fixed 256-bit integer.
///
/// That second bound is tight rather than conservative. The triangle's step is
/// sum-preserving, so after each level the values sum to exactly the running
/// denominator; every value, and every partial product the loop forms on the
/// way to one, is therefore bounded by the final denominator. The only thing
/// multiplied by anything larger afterwards is the apportionment's
/// `payout_scale * running`, which is the product checked here.
///
/// The denominator grows as `span^(d(d+1)/2)` — cubic in the span at degree 2,
/// and sixth power at degree 3. The private 256-bit capacity is why realistic
/// cubic bases now admit; a shape that exceeds even that fixed envelope still
/// refuses here, before founding, rather than trapping at settlement.
pub fn spline_arithmetic_envelope_v3<K: SplineKnotsV3 + ?Sized>(
    knots: &K,
    degree: u8,
    width: u32,
    payout_scale: u64,
) -> Result<()> {
    if !(BASIS_SPLINE_MINIMUM_DEGREE_V3..=BASIS_SPLINE_MAXIMUM_DEGREE_V3).contains(&degree) {
        return Err(Error::SplineDegreeOutOfProfile);
    }
    if payout_scale == 0 {
        return Err(Error::ZeroScale);
    }
    let degree = usize::from(degree);
    let width = usize::try_from(width).map_err(|_| Error::InvalidCount)?;
    let derived = knots
        .knot_count()
        .checked_sub(degree)
        .and_then(|value| value.checked_sub(1))
        .filter(|value| *value > 0)
        .ok_or(Error::SplineWidthDerivationMismatch)?;
    if derived != width {
        return Err(Error::SplineWidthDerivationMismatch);
    }

    let ceiling = SPLINE_COORDINATE_DENOMINATOR_CEILING_V3;
    let knot_at =
        |index: usize| -> Result<i128> { knots.knot_at(index).ok_or(Error::SplineDegenerateSpan) };
    // Evaluation carries each knot numerator onto the coordinate denominator
    // before locating a span. Bind that signed pre-scaling operation here as
    // well as the unsigned triangle below; otherwise a translated vector near
    // `i128::MAX` could pass the difference-only envelope and trap later even
    // though all of its spans were narrow.
    for index in 0..knots.knot_count() {
        knot_at(index)?
            .checked_mul(i128::from(ceiling))
            .ok_or(Error::SplineEnvelopeExceeded)?;
    }
    let mut selectable = 0_usize;
    for span in degree..width {
        // Only spans `locate_span` can actually return are worth bounding; the
        // rest are collapsed by interior multiplicity and never evaluated.
        if knot_at(span)? >= knot_at(step(span)?)? {
            continue;
        }
        selectable = step(selectable)?;
        let mut denominator = U256::from_u128(1);
        for level in 1..=degree {
            for offset in 0..level {
                let index = span
                    .checked_add(1)
                    .and_then(|shifted| shifted.checked_add(offset))
                    .and_then(|shifted| shifted.checked_sub(level))
                    .ok_or(Error::SplineDegenerateSpan)?;
                let high = knot_at(
                    index
                        .checked_add(level)
                        .ok_or(Error::SplineDegenerateSpan)?,
                )?;
                let support = nonnegative(
                    high.checked_sub(knot_at(index)?)
                        .ok_or(Error::ArithmeticOverflow)?,
                )?;
                if support == 0 {
                    // A span the coordinate can land in whose triangle divides
                    // by zero. Refused here rather than at settlement.
                    return Err(Error::SplineDegenerateSpan);
                }
                denominator = denominator
                    .checked_mul_u128(support)
                    .and_then(|value| value.checked_mul_u64(ceiling))
                    .map_err(|_| Error::SplineEnvelopeExceeded)?;
            }
        }
        denominator
            .checked_mul_u64(payout_scale)
            .map_err(|_| Error::SplineEnvelopeExceeded)?;
    }
    if selectable == 0 {
        return Err(Error::SplineDegenerateSpan);
    }
    Ok(())
}

/// The exact rational B-spline weights of one coordinate.
///
/// The weights are integer numerators over one common positive
/// [`denominator`](Self::denominator). No rounding decision has been taken at
/// this point; only the apportionment functions take one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplineWeightsV3 {
    /// Local de Boor numerators, `local_len` of them from index zero.
    local: [U256; SPLINE_MAX_SUPPORT_V3],
    /// Valid entries of `local`, always `degree + 1`.
    pub local_len: usize,
    /// First claim the local support covers: `span - degree`.
    pub offset: usize,
    /// Common positive denominator every entry of `local` is a numerator over.
    denominator: U256,
    /// Runtime claim width the weights scatter into.
    pub width: usize,
    /// The located non-degenerate knot span.
    pub span: usize,
}

impl SplineWeightsV3 {
    /// Whether one claim has exact zero weight.
    pub fn is_zero_at(&self, claim: usize) -> bool {
        self.wide_numerator_at(claim).is_zero()
    }

    /// The exact weight numerator when it fits the legacy `u128` inspection
    /// surface. Evaluation and apportionment themselves use the full fixed
    /// 256-bit value, so a refusal here cannot strand a live position.
    pub fn numerator_u128_at(&self, claim: usize) -> Result<u128> {
        self.wide_numerator_at(claim)
            .to_u128()
            .map_err(|_| Error::ArithmeticOverflow)
    }

    /// The exact common denominator when it fits the legacy `u128`
    /// inspection surface.
    pub fn denominator_u128(&self) -> Result<u128> {
        self.denominator
            .to_u128()
            .map_err(|_| Error::ArithmeticOverflow)
    }

    fn wide_numerator_at(&self, claim: usize) -> U256 {
        match claim.checked_sub(self.offset) {
            Some(local) if local < self.local_len => read(&self.local, local).unwrap_or(U256::ZERO),
            _ => U256::ZERO,
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
/// [`crate::payoff::runtime_v3`]'s business and is unchanged.
pub fn evaluate_spline_weights_v3<K: SplineKnotsV3 + ?Sized>(
    knots: &K,
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
        .knot_count()
        .checked_sub(degree)
        .and_then(|value| value.checked_sub(1))
        .filter(|value| *value > 0)
        .ok_or(Error::SplineWidthDerivationMismatch)?;
    if derived != width {
        return Err(Error::SplineWidthDerivationMismatch);
    }

    let scaled = |index: usize| -> Result<i128> {
        match knots.knot_at(index) {
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
    if coordinate_denominator > SPLINE_COORDINATE_DENOMINATOR_CEILING_V3 {
        return Err(Error::SplineCoordinateOutOfEnvelope);
    }
    // **Saturating, and exact.** The only thing this product is ever read
    // through is the clamp on the next line, whose two bounds are `scaled`
    // knots and so are exactly representable. Saturation is monotone, so it
    // preserves the order relation the clamp asks about: if the true product
    // exceeds `i128::MAX` it certainly exceeds `scaled(width)`, and
    // `i128::MAX` does too, so `min` picks the same branch either way. The
    // clamped value is therefore bit-identical to the one unbounded
    // arithmetic would produce -- this discards magnitude the computation was
    // going to discard anyway.
    //
    // A `checked_mul` here refused instead, which is how an oracle coordinate
    // far outside the knot range became a settlement-time trap on a Market
    // that had already taken money. That is the strand this saturation closes.
    let coordinate = coordinate_numerator.saturating_mul(i128::from(knot_denominator));
    // Below the domain the first claims pay their full weight, above it the
    // last ones do, rather than the coordinate falling off a half-open span.
    let clamped = scaled(degree)?.max(coordinate.min(scaled(width)?));
    let span = locate_span(&scaled, degree, width, clamped)?;
    let offset = span
        .checked_sub(degree)
        .ok_or(Error::SplineDegenerateSpan)?;

    let mut values = [U256::ZERO; SPLINE_MAX_SUPPORT_V3];
    write(&mut values, 0, U256::from_u128(1))?;
    let mut len = 1_usize;
    let mut denominator = U256::from_u128(1);
    for level in 1..=degree {
        let (numerators, denominators) = level_weights(&scaled, clamped, span, level)?;
        let suffix = suffix_products(&denominators, level)?;
        // One degree-raising step is a convex redistribution: under the weight
        // `p/q` each value sends `(q-p)*v` left and `p*v` right, and every
        // value already placed to the right is scaled by `q` so the level stays
        // over one common denominator. That is structurally sum-preserving,
        // which is why the partition of unity needs no reindexing argument.
        let mut raised = [U256::ZERO; SPLINE_MAX_SUPPORT_V3];
        for lower in (0..level).rev() {
            let numerator = read(&numerators, lower)?;
            let divisor = read(&denominators, lower)?;
            let right = step(lower)?;
            let carried = read(&suffix, right)?;
            let value = read(&values, lower)?;
            for higher in step(right)?..=level {
                let held = read(&raised, higher)?;
                write(
                    &mut raised,
                    higher,
                    checked_mul(U256::from_u128(divisor), held)?,
                )?;
            }
            let held = read(&raised, right)?;
            let sent_right = checked_mul(checked_mul(U256::from_u128(numerator), value)?, carried)?
                .checked_add(checked_mul(U256::from_u128(divisor), held)?)
                .map_err(|_| Error::ArithmeticOverflow)?;
            write(&mut raised, right, sent_right)?;
            // `numerator <= divisor` is structural: the level clamps it.
            let complement = divisor
                .checked_sub(numerator)
                .ok_or(Error::ArithmeticOverflow)?;
            let sent_left = checked_mul(checked_mul(U256::from_u128(complement), value)?, carried)?;
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
    let mut running = U256::ZERO;
    let mut carried = 0_u64;
    for claim in 0..weights.width {
        running = running
            .checked_add(weights.wide_numerator_at(claim))
            .map_err(|_| Error::ArithmeticOverflow)?;
        let boundary = floor_scaled_ratio(running, weights.denominator, payout_scale)?;
        let payout = boundary.checked_sub(carried).ok_or(Error::NonPartition)?;
        let slot = output.get_mut(claim).ok_or(Error::InvalidLength)?;
        *slot = payout;
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

fn preflight(weights: &SplineWeightsV3, payout_scale: u64, output: &[u64]) -> Result<()> {
    if payout_scale == 0 {
        return Err(Error::ZeroScale);
    }
    if weights.denominator.is_zero() {
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
) -> Result<[U256; SPLINE_MAX_SUPPORT_V3]> {
    let mut suffix = [U256::from_u128(1); SPLINE_MAX_SUPPORT_V3];
    for lower in (0..level).rev() {
        let above = read(&suffix, step(lower)?)?;
        let product = checked_mul(U256::from_u128(read(denominators, lower)?), above)?;
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

fn checked_mul(left: U256, right: U256) -> Result<U256> {
    left.checked_mul(right)
        .map_err(|_| Error::ArithmeticOverflow)
}

/// `floor(scale * numerator / denominator)` where `numerator <= denominator`.
///
/// The quotient is therefore in `[0, scale]`; binary search over that public
/// bound avoids a general-purpose big-integer division implementation while
/// preserving the one named cumulative-floor boundary exactly.
fn floor_scaled_ratio(numerator: U256, denominator: U256, scale: u64) -> Result<u64> {
    if denominator.is_zero() || numerator > denominator {
        return Err(Error::NonPartition);
    }
    let scaled = numerator
        .checked_mul_u64(scale)
        .map_err(|_| Error::ArithmeticOverflow)?;
    let mut low = 0_u64;
    let mut high = scale;
    while low < high {
        let delta = high.checked_sub(low).ok_or(Error::ArithmeticOverflow)?;
        let middle = low
            .checked_add(delta / 2)
            .and_then(|value| value.checked_add(delta % 2))
            .ok_or(Error::ArithmeticOverflow)?;
        if denominator
            .checked_mul_u64(middle)
            .map_err(|_| Error::ArithmeticOverflow)?
            <= scaled
        {
            low = middle;
        } else {
            high = middle.checked_sub(1).ok_or(Error::ArithmeticOverflow)?;
        }
    }
    Ok(low)
}

fn step(index: usize) -> Result<usize> {
    index.checked_add(1).ok_or(Error::ArithmeticOverflow)
}

/// Total read of one fixed-size triangle buffer. Every call site is
/// structurally in range; an out-of-range read is a fail-closed refusal rather
/// than a panic, so this module cannot abort.
fn read<T: Copy>(values: &[T], index: usize) -> Result<T> {
    values.get(index).copied().ok_or(Error::ArithmeticOverflow)
}

/// Total write into one fixed-size triangle buffer. See [`read`].
fn write<T>(values: &mut [T], index: usize, value: T) -> Result<()> {
    *values.get_mut(index).ok_or(Error::ArithmeticOverflow)? = value;
    Ok(())
}

#[cfg(test)]
mod wide_integer_tests {
    use super::*;

    #[test]
    fn multiplication_carries_across_every_64_bit_limb() {
        let low_128_max = U256([u64::MAX, u64::MAX, 0, 0]);
        assert_eq!(
            low_128_max.checked_mul(low_128_max),
            Ok(U256([1, 0, u64::MAX - 1, u64::MAX]))
        );
        let limb_max = U256([u64::MAX, 0, 0, 0]);
        assert_eq!(
            limb_max.checked_mul(limb_max),
            Ok(U256([1, u64::MAX - 1, 0, 0]))
        );
    }

    #[test]
    fn multiplication_refuses_instead_of_truncating_above_bit_255() {
        let maximum = U256([u64::MAX; 4]);
        assert_eq!(maximum.checked_mul_u64(1), Ok(maximum));
        assert!(maximum.checked_mul_u64(2).is_err());
        assert!(
            U256([0, 0, 0, 1_u64 << 63])
                .checked_mul(U256::from_u128(2))
                .is_err()
        );
    }

    #[test]
    fn floor_boundary_handles_the_full_closed_u64_range() {
        let denominator = U256([u64::MAX, u64::MAX, 7, 0]);
        assert_eq!(floor_scaled_ratio(U256::ZERO, denominator, u64::MAX), Ok(0));
        assert_eq!(
            floor_scaled_ratio(denominator, denominator, u64::MAX),
            Ok(u64::MAX)
        );
    }
}
