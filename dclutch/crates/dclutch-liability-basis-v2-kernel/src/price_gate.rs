//! The degree-`>= 2` arbitrage gate: integer hull membership over a basis.
//!
//! The semantics are owned by `DClutchSemantics.LiabilityBasisV2PriceGate` and
//! the byte record by `DClutchSemantics.LiabilityBasisV2PriceGateAbi`. This
//! module is an independent handwritten physical implementation of both,
//! checked byte for byte against the Lean-emitted corpora in
//! `generated_price_gate`.
//!
//! # Why this exists
//!
//! `LiabilityBasisV2` certifies the *claim* plane: payouts are nonnegative
//! integers summing to one collateral scale `Q`. At degree `<= 1` that makes
//! `p >= 0, sum p = Q` — the simplex condition — the whole no-arbitrage
//! condition on a price vector, because every claim attains a whole complete
//! set somewhere. At degree `>= 2` it stops being: an interior basis function
//! peaks at `3/4` (degree two) or `2/3` (degree three), so *three complete
//! sets, short four units of the interior claim* has a globally nonnegative
//! payoff and, at the simplex-admissible price `Q * e_j`, a strictly negative
//! price. That is an executable arbitrage.
//!
//! # What a certificate says
//!
//! That the price is a nonnegative integer mixture of *actually attainable*
//! payout vectors:
//!
//! ```text
//! 0 < W,  every weight positive,  sum weights = W
//! W * p_i = sum over atoms of weight * evaluate(coordinate)_i   for every claim i
//! ```
//!
//! Every atom is **recomputed** here by [`evaluate_spline_v2`] and never taken
//! off the wire. Lean's `Certificate.no_arbitrage` proves that a price with a
//! valid certificate admits no portfolio with a globally nonnegative payoff
//! and a negative price; `PhysicalAbi.decodeRequest_no_arbitrage` carries the
//! same statement across this byte boundary.
//!
//! # Three layers, in the order the verification runs them
//!
//! * **Decode and canonicalize.** [`decode_price_gate_v1`] applies the hostile
//!   checks in the exact order `PhysicalAbi.decodeChecks` lists them, up to the
//!   last check that does not need a basis. The first failing check names the
//!   refusal tag, so that order is part of the translation contract.
//! * **Bind.** The certificate repeats the scale, degree and width of the basis
//!   it is for, and those are compared against an *already authenticated*
//!   [`SplineRequestV2`] rather than against a digest of one. There is no hash
//!   preimage question and no second copy of the basis to disagree with.
//! * **Reconstruct.** Every named coordinate is evaluated through the
//!   production evaluator and the hull equation is checked componentwise in
//!   `u128`. No division, no rounding, no floating point.
//!
//! # Capacity, and the residual
//!
//! At most ten atoms, which is not an arbitrary capacity: every payout vector
//! lies in the affine hyperplane `sum = Q`, of dimension at most `width - 1`,
//! so affine Caratheodory bounds a hull point's support by `width`, and
//! `width <= SPLINE_MAX_WIDTH_V2 = 10`.
//!
//! The mass is a `u64`. A price inside the hull whose every representation
//! needs a larger common denominator is **refused**. That is a sufficient inner
//! certificate and it fails closed. Generation two carried the same residual
//! and named it; nothing here closes it.

use super::{
    Error, PRICE_GATE_ATOM_COUNT_OFFSET_V1, PRICE_GATE_DEGREE_OFFSET_V1,
    PRICE_GATE_DENOMINATOR_BYTES_V1, PRICE_GATE_DENOMINATORS_OFFSET_V1, PRICE_GATE_MAGIC_OFFSET_V1,
    PRICE_GATE_MAGIC_V1, PRICE_GATE_MASS_OFFSET_V1, PRICE_GATE_MAX_ATOMS_V1,
    PRICE_GATE_MAX_WIDTH_V1, PRICE_GATE_NUMERATOR_BYTES_V1, PRICE_GATE_NUMERATORS_OFFSET_V1,
    PRICE_GATE_PRICE_BYTES_V1, PRICE_GATE_PRICES_OFFSET_V1, PRICE_GATE_PROFILE_OFFSET_V1,
    PRICE_GATE_PROFILE_V1, PRICE_GATE_REQUEST_BYTES_V1, PRICE_GATE_RESERVED_BYTES_V1,
    PRICE_GATE_RESERVED_OFFSET_V1, PRICE_GATE_SCALE_OFFSET_V1, PRICE_GATE_SCHEMA_VERSION_V1,
    PRICE_GATE_VERSION_OFFSET_V1, PRICE_GATE_WEIGHT_BYTES_V1, PRICE_GATE_WEIGHTS_OFFSET_V1,
    PRICE_GATE_WIDTH_OFFSET_V1, Result, bytes, read_i64, read_u16, read_u32, read_u64, slice,
    spline::{
        SPLINE_MAX_DEGREE_V2, SPLINE_MIN_DEGREE_V2, SplineRequestV2, SplineWeightsV2,
        decode_spline_request_v2, evaluate_spline_v2,
    },
};

/// Highest B-spline degree exempt from the gate.
///
/// Not a physical capacity: it is the degree at which every claim still attains
/// a whole complete set, so `PriceGate.no_cap_of_attained_scale` has an
/// instance for every claim and the simplex condition is still the whole
/// no-arbitrage condition. LB-SPLINE pinned the attainment as
/// `hats.evaluate (at' 1 1) = [100, 0]`.
pub use crate::generated_price_gate::PRICE_GATE_EXEMPT_DEGREE_V1;

/// Hostile-decoded, canonicalized price-gate certificate.
///
/// Every structural fact the record alone can carry has been checked when one
/// of these exists. What remains is the pair of basis-dependent facts: that the
/// named coordinates are admitted, and that the hull equation closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceGateCertificateV1 {
    scale: u32,
    mass: u64,
    degree: u8,
    width: u8,
    atom_count: u8,
    prices: [u64; PRICE_GATE_MAX_WIDTH_V1],
    weights: [u64; PRICE_GATE_MAX_ATOMS_V1],
    numerators: [i64; PRICE_GATE_MAX_ATOMS_V1],
    denominators: [u32; PRICE_GATE_MAX_ATOMS_V1],
}

impl PriceGateCertificateV1 {
    /// Return the payout scale the price vector is denominated in.
    pub const fn scale(self) -> u32 {
        self.scale
    }

    /// Return the positive common denominator of the mixture weights.
    pub const fn mass(self) -> u64 {
        self.mass
    }

    /// Return the B-spline degree this certificate claims to be for.
    pub const fn degree(self) -> u8 {
        self.degree
    }

    /// Return the runtime claim width.
    pub fn width(self) -> usize {
        usize::from(self.width)
    }

    /// Return the sparse support size, always one through ten.
    pub fn atom_count(self) -> usize {
        usize::from(self.atom_count)
    }

    /// Return exactly the claimed price vector, without the canonical padding.
    pub fn active_prices(&self) -> &[u64] {
        self.prices.get(..self.width()).unwrap_or(&[])
    }

    /// Return exactly the active mixture weights.
    pub fn active_weights(&self) -> &[u64] {
        self.weights.get(..self.atom_count()).unwrap_or(&[])
    }
}

/// One admitted spline evaluation, with whatever certificate admitted it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedSplineEvaluationV2 {
    /// The exact integer payout partition of the request's own coordinate.
    pub weights: SplineWeightsV2,
    /// The verified certificate, when one was offered.
    pub certificate: Option<PriceGateCertificateV1>,
}

/// Decode exactly one canonical certificate and validate every structural fact
/// the record alone can carry, in the order the translation contract fixes.
///
/// The check order mirrors `PhysicalAbi.decodeChecks` position for position:
/// length, magic, schema, profile, reserved, scale, mass, degree, width, atom
/// count, padding, coordinate denominators, weights, coordinate order, weight
/// mass, primitive scale, price partition. The last three checks — the basis
/// binding, coordinate admission, and the hull equation — depend on a basis
/// rather than on the record, so they belong to [`verify_price_gate_v1`].
pub fn decode_price_gate_v1(input: &[u8]) -> Result<PriceGateCertificateV1> {
    if input.len() != PRICE_GATE_REQUEST_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    if bytes::<8>(input, PRICE_GATE_MAGIC_OFFSET_V1)? != PRICE_GATE_MAGIC_V1 {
        return Err(Error::InvalidMagic);
    }
    if read_u16(input, PRICE_GATE_VERSION_OFFSET_V1)? != PRICE_GATE_SCHEMA_VERSION_V1 {
        return Err(Error::UnsupportedSchema);
    }
    if read_u16(input, PRICE_GATE_PROFILE_OFFSET_V1)? != PRICE_GATE_PROFILE_V1 {
        return Err(Error::UnsupportedProfile);
    }
    if slice(
        input,
        PRICE_GATE_RESERVED_OFFSET_V1,
        PRICE_GATE_RESERVED_BYTES_V1,
    )?
    .iter()
    .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReserved);
    }
    let scale = read_u32(input, PRICE_GATE_SCALE_OFFSET_V1)?;
    if scale == 0 {
        return Err(Error::ZeroScale);
    }
    let mass = read_u64(input, PRICE_GATE_MASS_OFFSET_V1)?;
    if mass == 0 {
        return Err(Error::ZeroMass);
    }
    let degree = read_u8(input, PRICE_GATE_DEGREE_OFFSET_V1)?;
    if !(SPLINE_MIN_DEGREE_V2..=SPLINE_MAX_DEGREE_V2).contains(&degree) {
        return Err(Error::UnsupportedDegree);
    }
    let width = usize::from(read_u8(input, PRICE_GATE_WIDTH_OFFSET_V1)?);
    if width <= usize::from(degree) || width > PRICE_GATE_MAX_WIDTH_V1 {
        return Err(Error::WidthOutOfRange);
    }
    let atom_count = usize::from(read_u8(input, PRICE_GATE_ATOM_COUNT_OFFSET_V1)?);
    if atom_count == 0 || atom_count > PRICE_GATE_MAX_ATOMS_V1 {
        return Err(Error::AtomCountOutOfRange);
    }

    let mut prices = [0_u64; PRICE_GATE_MAX_WIDTH_V1];
    for (slot, price) in prices.iter_mut().enumerate() {
        *price = read_u64(
            input,
            slot_offset(PRICE_GATE_PRICES_OFFSET_V1, slot, PRICE_GATE_PRICE_BYTES_V1)?,
        )?;
    }
    let mut weights = [0_u64; PRICE_GATE_MAX_ATOMS_V1];
    let mut numerators = [0_i64; PRICE_GATE_MAX_ATOMS_V1];
    let mut denominators = [0_u32; PRICE_GATE_MAX_ATOMS_V1];
    for slot in 0..PRICE_GATE_MAX_ATOMS_V1 {
        let weight = read_u64(
            input,
            slot_offset(
                PRICE_GATE_WEIGHTS_OFFSET_V1,
                slot,
                PRICE_GATE_WEIGHT_BYTES_V1,
            )?,
        )?;
        let numerator = read_i64(
            input,
            slot_offset(
                PRICE_GATE_NUMERATORS_OFFSET_V1,
                slot,
                PRICE_GATE_NUMERATOR_BYTES_V1,
            )?,
        )?;
        let denominator = read_u32(
            input,
            slot_offset(
                PRICE_GATE_DENOMINATORS_OFFSET_V1,
                slot,
                PRICE_GATE_DENOMINATOR_BYTES_V1,
            )?,
        )?;
        write_u64(&mut weights, slot, weight)?;
        write_i64(&mut numerators, slot, numerator)?;
        write_u32(&mut denominators, slot, denominator)?;
    }

    // Canonical padding: one certificate has exactly one encoding.
    if prices.iter().skip(width).any(|price| *price != 0)
        || weights.iter().skip(atom_count).any(|weight| *weight != 0)
        || numerators
            .iter()
            .skip(atom_count)
            .any(|numerator| *numerator != 0)
        || denominators
            .iter()
            .skip(atom_count)
            .any(|denominator| *denominator != 0)
    {
        return Err(Error::NonCanonicalGatePadding);
    }
    if denominators
        .iter()
        .take(atom_count)
        .any(|denominator| *denominator == 0)
    {
        return Err(Error::ZeroDenominator);
    }
    if weights.iter().take(atom_count).any(|weight| *weight == 0) {
        return Err(Error::ZeroAtomWeight);
    }
    // Strictly increasing rational coordinates, by cross multiplication over
    // positive denominators. This forbids a repeated coordinate too, so one
    // support has one encoding.
    for slot in 1..atom_count {
        let previous = slot.checked_sub(1).ok_or(Error::NonCanonicalAtomOrder)?;
        let left = i128::from(read_i64_at(&numerators, previous)?)
            .checked_mul(i128::from(read_u32_at(&denominators, slot)?))
            .ok_or(Error::ArithmeticOverflow)?;
        let right = i128::from(read_i64_at(&numerators, slot)?)
            .checked_mul(i128::from(read_u32_at(&denominators, previous)?))
            .ok_or(Error::ArithmeticOverflow)?;
        if left >= right {
            return Err(Error::NonCanonicalAtomOrder);
        }
    }
    let mut weight_sum = 0_u128;
    let mut divisor = mass;
    for weight in weights.iter().take(atom_count) {
        weight_sum = weight_sum
            .checked_add(u128::from(*weight))
            .ok_or(Error::ArithmeticOverflow)?;
        divisor = gcd(divisor, *weight);
    }
    if weight_sum != u128::from(mass) {
        return Err(Error::WeightMassMismatch);
    }
    // Scaling the weights and the mass together leaves the hull equation
    // unchanged, so the boundary admits only the primitive representative.
    // This canonicalizes the scale; it does NOT make the support unique.
    if divisor != 1 {
        return Err(Error::NonPrimitiveWeightScale);
    }
    let mut price_sum = 0_u128;
    for price in prices.iter().take(width) {
        price_sum = price_sum
            .checked_add(u128::from(*price))
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != u128::from(scale) {
        return Err(Error::PriceNotPartition);
    }
    Ok(PriceGateCertificateV1 {
        scale,
        mass,
        degree,
        width: read_u8(input, PRICE_GATE_WIDTH_OFFSET_V1)?,
        atom_count: read_u8(input, PRICE_GATE_ATOM_COUNT_OFFSET_V1)?,
        prices,
        weights,
        numerators,
        denominators,
    })
}

/// Verify one canonical certificate against one already authenticated basis.
///
/// The basis must come from an authenticated source; this function does not
/// authenticate it, and a certificate verified against an attacker-chosen
/// basis certifies nothing about a Market.
pub fn verify_price_gate_v1(
    basis: &SplineRequestV2,
    input: &[u8],
) -> Result<PriceGateCertificateV1> {
    let certificate = decode_price_gate_v1(input)?;
    if u64::from(certificate.scale) != u64::from(basis.scale())
        || certificate.degree != basis.degree()
        || certificate.width() != basis.width()
    {
        return Err(Error::PriceGateBasisMismatch);
    }
    let mut mixture = [0_u128; PRICE_GATE_MAX_WIDTH_V1];
    for slot in 0..certificate.atom_count() {
        // Every atom is recomputed here. Nothing about a payout vector is ever
        // taken from the certificate.
        let atom = evaluate_spline_v2(&basis.with_coordinate(
            read_i64_at(&certificate.numerators, slot)?,
            read_u32_at(&certificate.denominators, slot)?,
        )?)?;
        let weight = u128::from(read_u64_at(&certificate.weights, slot)?);
        for (claim, payout) in atom.active().iter().enumerate() {
            let weighted = weight
                .checked_mul(u128::from(*payout))
                .ok_or(Error::ArithmeticOverflow)?;
            let slot = mixture
                .get_mut(claim)
                .ok_or(Error::PriceReconstructionMismatch)?;
            *slot = slot
                .checked_add(weighted)
                .ok_or(Error::ArithmeticOverflow)?;
        }
    }
    for claim in 0..certificate.width() {
        let scaled = u128::from(read_u64_at(&certificate.prices, claim)?)
            .checked_mul(u128::from(certificate.mass))
            .ok_or(Error::ArithmeticOverflow)?;
        if scaled
            != *mixture
                .get(claim)
                .ok_or(Error::PriceReconstructionMismatch)?
        {
            return Err(Error::PriceReconstructionMismatch);
        }
    }
    Ok(certificate)
}

/// **The admission conjunct.** Decode one spline request, apply the
/// degree-`>= 2` gate, and evaluate.
///
/// A request of degree above [`PRICE_GATE_EXEMPT_DEGREE_V1`] is evaluated for
/// sale only alongside a certificate this gate accepts against that same
/// request; without one it is refused with [`Error::PriceGateRequired`]. Degree
/// at or below the exempt degree needs none — but a certificate that *is*
/// offered is verified regardless of degree, so an input that is present is
/// never silently ignored.
///
/// Lean: `PhysicalAbi.admitEvaluation`, and
/// `admitEvaluation_refuses_graded_without_certificate` is the theorem that
/// nothing at degree `>= 2` gets through the first arm.
pub fn admit_and_evaluate_spline_v2(
    request: &[u8],
    certificate: Option<&[u8]>,
) -> Result<AdmittedSplineEvaluationV2> {
    let basis = decode_spline_request_v2(request)?;
    let verified = match certificate {
        Some(input) => Some(verify_price_gate_v1(&basis, input)?),
        None => {
            if basis.degree() > PRICE_GATE_EXEMPT_DEGREE_V1 {
                return Err(Error::PriceGateRequired);
            }
            None
        }
    };
    Ok(AdmittedSplineEvaluationV2 {
        weights: evaluate_spline_v2(&basis)?,
        certificate: verified,
    })
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn slot_offset(base: usize, slot: usize, width: usize) -> Result<usize> {
    base.checked_add(slot.checked_mul(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

/// Total reads and writes of the fixed-size slot arrays. Every call site is
/// structurally in range; an out-of-range access is a fail-closed refusal
/// rather than a panic, so this module cannot abort.
fn read_u64_at(values: &[u64], index: usize) -> Result<u64> {
    values.get(index).copied().ok_or(Error::ArithmeticOverflow)
}

fn read_i64_at(values: &[i64], index: usize) -> Result<i64> {
    values.get(index).copied().ok_or(Error::ArithmeticOverflow)
}

fn read_u32_at(values: &[u32], index: usize) -> Result<u32> {
    values.get(index).copied().ok_or(Error::ArithmeticOverflow)
}

fn write_u64(values: &mut [u64], index: usize, value: u64) -> Result<()> {
    *values.get_mut(index).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn write_i64(values: &mut [i64], index: usize, value: i64) -> Result<()> {
    *values.get_mut(index).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn write_u32(values: &mut [u32], index: usize, value: u32) -> Result<()> {
    *values.get_mut(index).ok_or(Error::InvalidLength)? = value;
    Ok(())
}
