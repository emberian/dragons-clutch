//! Degree-one through degree-three B-spline liability bases.
//!
//! The semantics are owned by `DClutchSemantics.LiabilityBasisV2Spline` and the
//! byte record by `DClutchSemantics.LiabilityBasisV2SplineAbi`. This module is
//! an independent handwritten physical implementation of both, checked byte for
//! byte against the Lean-emitted corpora in `generated_spline`.
//!
//! Three layers, in the order the evaluation runs them.
//!
//! * **Decode.** [`decode_spline_request_v2`] applies the hostile checks in the
//!   exact order `PhysicalAbi.decodeChecks` lists them. The first failing check
//!   names the refusal tag, so that order is part of the translation contract.
//! * **Evaluate.** [`evaluate_spline_basis_v2`] runs Cox-de-Boor on integers:
//!   every basis value is a numerator over one accumulating denominator, so no
//!   rational division and no floating point occurs. Only the `degree + 1`
//!   locally supported claims are computed; every other claim's weight is an
//!   exact zero rather than a rounded one.
//! * **Apportion.** [`apportion_spline_v2`] is the single named rounding
//!   boundary: it floors the *running* weight sum into the collateral scale and
//!   hands each claim the difference between two consecutive floors. Because
//!   the weights sum to the denominator, the final floor is exactly the scale,
//!   so the payouts telescope to it with no remainder step and no residue.
//!
//! **Overflow has no Lean counterpart.** Lean quantifies over unbounded `Nat`
//! and `Int`, so it never refuses for size. This kernel evaluates inside a
//! `u128`/`i128` envelope and refuses with [`Error::ArithmeticOverflow`] the
//! moment a checked operation would leave it. That refusal is a property of the
//! physical profile alone; no generated corpus case reaches it, and no theorem
//! in `LiabilityBasisV2Spline` is weakened by it. The same fail-closed tag also
//! covers the structurally unreachable bounds misses in the fixed-size triangle
//! buffers, so nothing in this module can panic.

use super::{
    Error, Result, SPLINE_COORDINATE_DENOMINATOR_OFFSET_V2, SPLINE_COORDINATE_NUMERATOR_OFFSET_V2,
    SPLINE_DEGREE_OFFSET_V2, SPLINE_KNOT_BYTES_V2, SPLINE_KNOT_COUNT_OFFSET_V2,
    SPLINE_KNOT_DENOMINATOR_OFFSET_V2, SPLINE_KNOTS_OFFSET_V2, SPLINE_MAGIC_OFFSET_V2,
    SPLINE_MAGIC_V2, SPLINE_MAX_KNOTS_V2, SPLINE_MAX_WIDTH_V2, SPLINE_PROFILE_OFFSET_V2,
    SPLINE_PROFILE_V2, SPLINE_REQUEST_BYTES_V2, SPLINE_RESERVED_BYTES_V2,
    SPLINE_RESERVED_OFFSET_V2, SPLINE_SCALE_OFFSET_V2, SPLINE_SCHEMA_VERSION_V2,
    SPLINE_VERSION_OFFSET_V2, bytes, read_i64, read_u16, read_u32, slice,
};

/// Lowest B-spline degree this physical profile admits.
///
/// Degree zero is the categorical one-hot basis, which `LiabilityBasisV2`
/// already instantiates separately, so this record does not re-express it.
pub const SPLINE_MIN_DEGREE_V2: u8 = 1;

/// Highest B-spline degree this physical profile admits.
///
/// A physical capacity of the record, not a mathematical bound: `SplineProfile`
/// itself is stated for every degree.
pub const SPLINE_MAX_DEGREE_V2: u8 = 3;

/// Claims one coordinate can be locally supported on: `degree + 1`.
pub const SPLINE_MAX_SUPPORT_V2: usize = 4;

/// Weights one Cox-de-Boor level carries at most: `degree` of them.
const LEVEL_CAPACITY: usize = SPLINE_MAX_SUPPORT_V2 - 1;

/// Hostile-decoded provisional B-spline request.
///
/// Every structural fact `PhysicalAbi.Request.WellFormed` names has already
/// been checked when one of these exists. The remaining refusal is the
/// coordinate-dependent one: whether the located span is a real span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplineRequestV2 {
    scale: u32,
    knot_denominator: u32,
    coordinate_denominator: u32,
    coordinate_numerator: i64,
    degree: u8,
    knot_count: u8,
    knots: [i64; SPLINE_MAX_KNOTS_V2],
}

impl SplineRequestV2 {
    /// Return the positive integer payout scale `Q`.
    pub const fn scale(self) -> u32 {
        self.scale
    }

    /// Return the positive common denominator of the knot numerators.
    pub const fn knot_denominator(self) -> u32 {
        self.knot_denominator
    }

    /// Return the positive denominator of the evaluated coordinate.
    pub const fn coordinate_denominator(self) -> u32 {
        self.coordinate_denominator
    }

    /// Return the signed numerator of the evaluated coordinate.
    pub const fn coordinate_numerator(self) -> i64 {
        self.coordinate_numerator
    }

    /// Return the B-spline degree, always one through three.
    pub const fn degree(self) -> u8 {
        self.degree
    }

    /// Return the active knot count, always `2 * degree + 2` through twelve.
    pub const fn knot_count(self) -> u8 {
        self.knot_count
    }

    /// Return the runtime claim width `K = knot_count - degree - 1`.
    ///
    /// Lean: `PhysicalAbi.Request.width`. The decoder's knot-count check makes
    /// this at least `degree + 1` and at most [`SPLINE_MAX_WIDTH_V2`].
    pub fn width(self) -> usize {
        usize::from(self.knot_count).saturating_sub(usize::from(self.degree).saturating_add(1))
    }

    /// Return exactly the knots the basis uses, without the canonical padding.
    ///
    /// Lean: `PhysicalAbi.Request.activeKnots`.
    pub fn active_knots(&self) -> &[i64] {
        self.knots
            .get(..usize::from(self.knot_count))
            .unwrap_or(&[])
    }
}

/// The exact rational B-spline weights of one admitted request.
///
/// The weights are integer numerators over one common positive
/// [`denominator`](Self::denominator), which is the form
/// `SplineProfile.localNumerators` and `SplineProfile.basisDenominator` carry.
/// No rounding decision has been taken at this point; only
/// [`apportion_spline_v2`] takes one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplineBasisV2 {
    /// Local de Boor numerators, `local_len` of them starting at index zero.
    pub local: [u128; SPLINE_MAX_SUPPORT_V2],
    /// Valid entries of `local`, always `degree + 1`.
    pub local_len: usize,
    /// First claim coordinate the local support covers: `span - degree`.
    pub offset: usize,
    /// Common positive denominator every entry of `local` is a numerator over.
    pub denominator: u128,
    /// Runtime claim width the weights are scattered into.
    pub width: usize,
    /// The located non-degenerate knot span.
    pub span: usize,
}

impl SplineBasisV2 {
    /// Return the exact numerator of one claim's weight.
    ///
    /// Claims outside the local support carry an exact zero, which is what
    /// `SplineProfile.scatter` places and what
    /// `SplineProfile.evaluate_zero_outside_support` keeps through the
    /// apportionment.
    pub fn numerator_at(&self, claim: usize) -> u128 {
        match claim.checked_sub(self.offset) {
            Some(local) if local < self.local_len => read(&self.local, local).unwrap_or(0),
            _ => 0,
        }
    }
}

/// One evaluated runtime-width payout partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplineWeightsV2 {
    /// Runtime claim width; entries of `payouts` beyond it are zero.
    pub width: usize,
    /// Exact integer payouts, zero padded to the physical capacity.
    pub payouts: [u64; SPLINE_MAX_WIDTH_V2],
}

impl SplineWeightsV2 {
    /// Return exactly the runtime-width payout vector.
    pub fn active(&self) -> &[u64] {
        self.payouts.get(..self.width).unwrap_or(&[])
    }
}

/// Decode exactly one canonical B-spline request and validate every structural
/// fact, in the order the translation contract fixes.
///
/// The check order mirrors `PhysicalAbi.decodeChecks` position for position:
/// length, magic, schema, profile, reserved bytes, scale, denominators, degree,
/// knot count, knot padding, knot order. The twelfth check — whether the
/// located span is a real one — depends on the coordinate rather than on the
/// record, so it belongs to [`evaluate_spline_basis_v2`].
pub fn decode_spline_request_v2(input: &[u8]) -> Result<SplineRequestV2> {
    if input.len() != SPLINE_REQUEST_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    if bytes::<8>(input, SPLINE_MAGIC_OFFSET_V2)? != SPLINE_MAGIC_V2 {
        return Err(Error::InvalidMagic);
    }
    if read_u16(input, SPLINE_VERSION_OFFSET_V2)? != SPLINE_SCHEMA_VERSION_V2 {
        return Err(Error::UnsupportedSchema);
    }
    if read_u16(input, SPLINE_PROFILE_OFFSET_V2)? != SPLINE_PROFILE_V2 {
        return Err(Error::UnsupportedProfile);
    }
    if slice(input, SPLINE_RESERVED_OFFSET_V2, SPLINE_RESERVED_BYTES_V2)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReserved);
    }
    let scale = read_u32(input, SPLINE_SCALE_OFFSET_V2)?;
    if scale == 0 {
        return Err(Error::ZeroScale);
    }
    let knot_denominator = read_u32(input, SPLINE_KNOT_DENOMINATOR_OFFSET_V2)?;
    let coordinate_denominator = read_u32(input, SPLINE_COORDINATE_DENOMINATOR_OFFSET_V2)?;
    if knot_denominator == 0 || coordinate_denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    let degree = read_u8(input, SPLINE_DEGREE_OFFSET_V2)?;
    if !(SPLINE_MIN_DEGREE_V2..=SPLINE_MAX_DEGREE_V2).contains(&degree) {
        return Err(Error::UnsupportedDegree);
    }
    let knot_count = usize::from(read_u8(input, SPLINE_KNOT_COUNT_OFFSET_V2)?);
    let least_knots = usize::from(degree)
        .checked_mul(2)
        .and_then(|doubled| doubled.checked_add(2))
        .ok_or(Error::KnotCountOutOfRange)?;
    if !(least_knots..=SPLINE_MAX_KNOTS_V2).contains(&knot_count) {
        return Err(Error::KnotCountOutOfRange);
    }
    if knot_padding(input, knot_count)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalKnotPadding);
    }
    let mut knots = [0_i64; SPLINE_MAX_KNOTS_V2];
    for (slot, knot) in knots.iter_mut().enumerate() {
        let offset = SPLINE_KNOTS_OFFSET_V2
            .checked_add(
                slot.checked_mul(SPLINE_KNOT_BYTES_V2)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        *knot = read_i64(input, offset)?;
    }
    for index in 1..knot_count {
        let previous = read_knot(&knots, index.checked_sub(1).ok_or(Error::KnotsNotOrdered)?)?;
        if previous > read_knot(&knots, index)? {
            return Err(Error::KnotsNotOrdered);
        }
    }
    Ok(SplineRequestV2 {
        scale,
        knot_denominator,
        coordinate_denominator,
        coordinate_numerator: read_i64(input, SPLINE_COORDINATE_NUMERATOR_OFFSET_V2)?,
        degree,
        knot_count: read_u8(input, SPLINE_KNOT_COUNT_OFFSET_V2)?,
        knots,
    })
}

/// Evaluate the exact rational B-spline weights of one decoded request.
///
/// This is integer Cox-de-Boor. Both scaled coordinate and scaled knots are
/// carried over the single common denominator `knot_denominator *
/// coordinate_denominator`, so the located span, the clamp and every de Boor
/// weight are exact integer comparisons rather than rational ones.
///
/// Refuses [`Error::DegenerateSpan`] when the record names no non-degenerate
/// span at all, when the located span leaves the domain, or when any de Boor
/// denominator collapses — the three conjuncts of `SplineProfile.admits`
/// beyond the coordinate denominator the decoder already checked.
pub fn evaluate_spline_basis_v2(request: &SplineRequestV2) -> Result<SplineBasisV2> {
    let degree = usize::from(request.degree);
    let width = request.width();
    if width == 0 || width > SPLINE_MAX_WIDTH_V2 {
        return Err(Error::KnotCountOutOfRange);
    }
    let scaled = scaled_knots(request)?;
    let coordinate = i128::from(request.coordinate_numerator)
        .checked_mul(i128::from(request.knot_denominator))
        .ok_or(Error::ArithmeticOverflow)?;
    // Lean: `SplineProfile.clampedCoordinate`. Below the domain the first
    // claims pay their full weight, above it the last ones do, rather than the
    // coordinate falling off a half-open span.
    let clamped = knot_at(&scaled, degree).max(coordinate.min(knot_at(&scaled, width)));
    let span = locate_span(&scaled, degree, width, clamped)?;
    let offset = span.checked_sub(degree).ok_or(Error::DegenerateSpan)?;

    let mut values = [0_u128; SPLINE_MAX_SUPPORT_V2];
    write(&mut values, 0, 1)?;
    let mut len = 1_usize;
    let mut denominator = 1_u128;
    for level in 1..=degree {
        let (numerators, denominators) = level_weights(&scaled, clamped, span, level)?;
        let suffix = suffix_products(&denominators, level)?;
        // One degree-raising step is a convex redistribution: under the weight
        // `p/q` each value sends `(q-p)*v` left and `p*v` right, and every
        // value already placed to the right is scaled by `q` so that the level
        // stays over one common denominator. That is structurally
        // sum-preserving, which is why the partition of unity needs no
        // reindexing argument here or in Lean.
        //
        // `raised[level]` starts at zero, which is Lean's `deBoorStep [] _`
        // base case; each step then consumes `values[lower]` into the two
        // claims either side of it.
        let mut raised = [0_u128; SPLINE_MAX_SUPPORT_V2];
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
        return Err(Error::DegenerateSpan);
    }
    Ok(SplineBasisV2 {
        local: values,
        local_len: len,
        offset,
        denominator,
        width,
        span,
    })
}

/// Apportion `scale` collateral atoms across exact rational B-spline weights.
///
/// **The sole rounding boundary of this profile.** Lean:
/// `Spline.cumulativeFloorBoundaryV2` and `Spline.apportion`. Each claim
/// receives the difference between two consecutive floors of the *running*
/// weight sum, never a floor of its own weight plus a remainder distribution.
/// Since the weights sum to the denominator, the final floor is exactly the
/// scale and the payouts telescope to it: `sum(payouts) == scale` holds
/// exactly, at every width, with no residue atom to classify.
pub fn apportion_spline_v2(basis: &SplineBasisV2, scale: u32) -> Result<SplineWeightsV2> {
    if scale == 0 {
        return Err(Error::ZeroScale);
    }
    if basis.denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    if basis.width == 0 {
        return Err(Error::EmptyBasis);
    }
    if basis.width > SPLINE_MAX_WIDTH_V2 {
        return Err(Error::WidthMismatch);
    }
    let mut payouts = [0_u64; SPLINE_MAX_WIDTH_V2];
    let mut running = 0_u128;
    let mut carried = 0_u128;
    for claim in 0..basis.width {
        running = running
            .checked_add(basis.numerator_at(claim))
            .ok_or(Error::ArithmeticOverflow)?;
        let boundary = u128::from(scale)
            .checked_mul(running)
            .ok_or(Error::ArithmeticOverflow)?
            .checked_div(basis.denominator)
            .ok_or(Error::ZeroDenominator)?;
        let payout = boundary.checked_sub(carried).ok_or(Error::NonPartition)?;
        write_payout(
            &mut payouts,
            claim,
            u64::try_from(payout).map_err(|_| Error::ArithmeticOverflow)?,
        )?;
        carried = boundary;
    }
    // Redundant fail-closed assertion rather than a reachable refusal: Lean
    // derives the partition of unity structurally from the de Boor step
    // (`deBoorLevels_sum`), so a triangle that reaches here always sums to its
    // own denominator.
    if running != basis.denominator {
        return Err(Error::NonPartition);
    }
    Ok(SplineWeightsV2 {
        width: basis.width,
        payouts,
    })
}

/// Evaluate one decoded request into its exact integer payout partition.
pub fn evaluate_spline_v2(request: &SplineRequestV2) -> Result<SplineWeightsV2> {
    apportion_spline_v2(&evaluate_spline_basis_v2(request)?, request.scale)
}

/// Decode and evaluate one canonical B-spline request record.
///
/// This is the whole physical boundary: bytes in, an exact nonnegative integer
/// partition of the named scale out, or one stable [`Error::tag`].
pub fn decode_and_evaluate_spline_v2(input: &[u8]) -> Result<SplineWeightsV2> {
    evaluate_spline_v2(&decode_spline_request_v2(input)?)
}

/// Scale every active knot onto the coordinate's denominator, once.
///
/// Slots at and beyond the active knot count read zero, matching Lean's total
/// `knotAt`; the decoder has separately required those slots to be canonical
/// zero bytes, so the two agree on every accepted record.
fn scaled_knots(request: &SplineRequestV2) -> Result<[i128; SPLINE_MAX_KNOTS_V2]> {
    let mut scaled = [0_i128; SPLINE_MAX_KNOTS_V2];
    let active = usize::from(request.knot_count);
    for (slot, knot) in scaled.iter_mut().zip(request.knots.iter()).take(active) {
        *slot = i128::from(*knot)
            .checked_mul(i128::from(request.coordinate_denominator))
            .ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(scaled)
}

/// The non-degenerate span carrying the clamped coordinate.
///
/// Lean: `SplineProfile.spanCandidates` and `SplineProfile.locateSpan`. A knot
/// of multiplicity `r` collapses `r - 1` spans; those are skipped here rather
/// than refused, which is exactly what admits interior multiplicity — and so a
/// corner or a jump inside an otherwise smooth basis — at all.
fn locate_span(
    scaled: &[i128; SPLINE_MAX_KNOTS_V2],
    degree: usize,
    width: usize,
    clamped: i128,
) -> Result<usize> {
    let mut first = None;
    let mut located = None;
    for span in degree..width {
        let low = knot_at(scaled, span);
        let high = knot_at(scaled, span.checked_add(1).ok_or(Error::DegenerateSpan)?);
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
    let span = located.or(first).ok_or(Error::DegenerateSpan)?;
    if span < degree || span >= width {
        return Err(Error::DegenerateSpan);
    }
    Ok(span)
}

/// The `level` Cox-de-Boor weights of one triangle level, as numerator and
/// denominator pairs at knot indices `span + 1 - level` through `span`.
///
/// Lean: `SplineProfile.levelWeights`. The numerator is clamped to the
/// denominator so no weight can exceed one; on a correctly located span that
/// clamp is inert, and keeping it is what makes `numerator <= denominator`
/// structural rather than a lemma this kernel would have to assume.
fn level_weights(
    scaled: &[i128; SPLINE_MAX_KNOTS_V2],
    clamped: i128,
    span: usize,
    level: usize,
) -> Result<([u128; LEVEL_CAPACITY], [u128; LEVEL_CAPACITY])> {
    let mut numerators = [0_u128; LEVEL_CAPACITY];
    let mut denominators = [0_u128; LEVEL_CAPACITY];
    for (offset, (numerator, denominator)) in numerators
        .iter_mut()
        .zip(denominators.iter_mut())
        .enumerate()
        .take(level)
    {
        // `level <= degree <= span`, so this never underflows.
        let index = span
            .checked_add(1)
            .and_then(|shifted| shifted.checked_add(offset))
            .and_then(|shifted| shifted.checked_sub(level))
            .ok_or(Error::DegenerateSpan)?;
        let base = knot_at(scaled, index);
        let support = nonnegative(
            knot_at(
                scaled,
                index.checked_add(level).ok_or(Error::DegenerateSpan)?,
            )
            .checked_sub(base)
            .ok_or(Error::ArithmeticOverflow)?,
        )?;
        if support == 0 {
            return Err(Error::DegenerateSpan);
        }
        let elapsed = nonnegative(clamped.checked_sub(base).ok_or(Error::ArithmeticOverflow)?)?;
        *numerator = elapsed.min(support);
        *denominator = support;
    }
    Ok((numerators, denominators))
}

/// Suffix products of one level's denominators, `suffix[level] == 1`.
///
/// Lean: `weightProduct` of the level's tail. `suffix[0]` is the factor the
/// whole triangle's common denominator picks up from this level.
fn suffix_products(
    denominators: &[u128; LEVEL_CAPACITY],
    level: usize,
) -> Result<[u128; SPLINE_MAX_SUPPORT_V2]> {
    let mut suffix = [1_u128; SPLINE_MAX_SUPPORT_V2];
    for lower in (0..level).rev() {
        let above = read(&suffix, step(lower)?)?;
        let product = checked_mul(read(denominators, lower)?, above)?;
        write(&mut suffix, lower, product)?;
    }
    Ok(suffix)
}

/// Total scaled-knot read. An out-of-range index reads zero.
///
/// Lean: `knotAt`.
fn knot_at(scaled: &[i128; SPLINE_MAX_KNOTS_V2], index: usize) -> i128 {
    match scaled.get(index) {
        Some(value) => *value,
        None => 0,
    }
}

/// Clamp one exact signed difference to a nonnegative magnitude.
///
/// Lean: `Int.toNat`, which is `max 0` rather than a truncation.
fn nonnegative(value: i128) -> Result<u128> {
    if value <= 0 {
        return Ok(0);
    }
    u128::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

/// The inactive knot slots, which must be canonical zero.
///
/// Lean: `PhysicalAbi.knotPadding`.
fn knot_padding(input: &[u8], knot_count: usize) -> Result<&[u8]> {
    let used = knot_count
        .checked_mul(SPLINE_KNOT_BYTES_V2)
        .ok_or(Error::NonCanonicalKnotPadding)?;
    let start = SPLINE_KNOTS_OFFSET_V2
        .checked_add(used)
        .ok_or(Error::NonCanonicalKnotPadding)?;
    let width = SPLINE_MAX_KNOTS_V2
        .checked_sub(knot_count)
        .and_then(|slots| slots.checked_mul(SPLINE_KNOT_BYTES_V2))
        .ok_or(Error::NonCanonicalKnotPadding)?;
    slice(input, start, width)
}

/// One checked exact product inside the physical envelope.
fn checked_mul(left: u128, right: u128) -> Result<u128> {
    left.checked_mul(right).ok_or(Error::ArithmeticOverflow)
}

/// One checked index step. Structurally in range at every call site.
fn step(index: usize) -> Result<usize> {
    index.checked_add(1).ok_or(Error::ArithmeticOverflow)
}

/// Total read of one fixed-size triangle buffer.
///
/// Every call site is structurally in range; an out-of-range read is a
/// fail-closed refusal rather than a panic, so this crate cannot abort.
fn read(values: &[u128], index: usize) -> Result<u128> {
    values.get(index).copied().ok_or(Error::ArithmeticOverflow)
}

/// Total write into one fixed-size triangle buffer. See [`read`].
fn write(values: &mut [u128], index: usize, value: u128) -> Result<()> {
    *values.get_mut(index).ok_or(Error::ArithmeticOverflow)? = value;
    Ok(())
}

/// Total read of one decoded knot slot. See [`read`].
fn read_knot(knots: &[i64; SPLINE_MAX_KNOTS_V2], index: usize) -> Result<i64> {
    knots.get(index).copied().ok_or(Error::KnotsNotOrdered)
}

/// Total write into the runtime-width payout vector. See [`read`].
fn write_payout(payouts: &mut [u64; SPLINE_MAX_WIDTH_V2], index: usize, value: u64) -> Result<()> {
    *payouts.get_mut(index).ok_or(Error::WidthMismatch)? = value;
    Ok(())
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}
