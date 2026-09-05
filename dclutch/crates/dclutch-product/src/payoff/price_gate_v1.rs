//! The `DCLTPGT1` no-arbitrage price certificate, decoded and verified in the
//! crate that owns the live basis wire.
//!
//! # Why this is a port and not a call
//!
//! `BASIS_ABI_UNIFICATION_V1` §6.3 puts the degree-`>= 2` price gate at
//! **founding**, once, and never on the hot path. Founding runs in
//! `dclutch-core-sbf`, which reaches the basis through this crate — so the
//! certificate has to be decodable from here.
//!
//! It cannot be decoded through `dclutch-liability-basis-v2-kernel`, where the
//! original implementation lives. Under the option-D ruling that kernel is
//! retained as a **non-authoritative differential reference** only, and it is
//! wired as a *dev-dependency* of this crate precisely so it stays one —
//! "linked into the test binary and into no ELF". Reaching into it from a
//! founding route would make it a second live writer and break `O-005`.
//!
//! So the decoder and the hull check are ported, exactly as the de Boor
//! evaluator was, and the kernel stays the differential reference that says the
//! port got it right. The **specification** is not ported: the ABI constants
//! come from the same Lean owner that authors the kernel's, emitted a second
//! time into `generated_price_gate_v1.rs` and byte-guarded there. §1.6.1
//! measured what happens when one wire tag acquires independent authors, and
//! this is the shape that avoids it.
//!
//! # What the certificate asserts, and why recomputation is the whole point
//!
//! A certificate claims that the basis's price vector lies in the convex hull
//! of payout vectors the basis itself produces at a finite set of coordinates:
//!
//! ```text
//!     price_j * mass == sum over atoms i of ( weight_i * payout_i_j )
//! ```
//!
//! for every claim `j`, with `sum(weight_i) == mass`. At degree `<= 1` the
//! simplex condition *is* the no-arbitrage condition and this is unnecessary;
//! at degree `>= 2` it stops being, and a basis without this witness admits an
//! executable arbitrage (`EXPANSION_FRONTIER_2026_08_25` §"Slice two").
//!
//! **Every `payout_i_j` is recomputed here, through the production evaluator.**
//! Nothing about a payout vector is ever taken from the certificate — the
//! certificate supplies only *where* to look (the atom coordinates) and *how
//! much* to weight each. That is what makes a forged certificate useless: it
//! can name any coordinates it likes, and the payouts it gets are the ones the
//! basis actually produces.
//!
//! # Capacity 10 is a theorem, not a cap
//!
//! `PRICE_GATE_MAX_ATOMS_V1 = 10` is affine Carathéodory: a point in the convex
//! hull of a `width`-dimensional simplex needs at most `width` atoms to witness
//! it. It is emitted from Lean alongside the offsets rather than chosen here.

use crate::payoff::runtime_v3::{Error, Result};
use crate::payoff::spline_eval_v3::{
    SplineKnotsV3, apportion_cumulative_v3, evaluate_spline_weights_v3,
};

#[allow(missing_docs)]
mod generated {
    include!("generated_price_gate_v1.rs");
}

pub use generated::*;

/// One decoded, structurally-valid price-gate certificate.
///
/// Every field is private. A caller that could reach in and read `prices`
/// without having run [`verify_price_gate_v1`] would be reading an
/// attacker-supplied number that looks like a verified one.
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
    /// Payout scale the certificate's prices partition.
    pub fn scale(self) -> u32 {
        self.scale
    }

    /// Common denominator of the atom weights.
    pub fn mass(self) -> u64 {
        self.mass
    }

    /// Spline degree this certificate was issued against.
    pub fn degree(self) -> u8 {
        self.degree
    }

    /// Basis width this certificate was issued against.
    pub fn width(self) -> usize {
        usize::from(self.width)
    }

    /// How many hull atoms the certificate carries.
    pub fn atom_count(self) -> usize {
        usize::from(self.atom_count)
    }

    /// The certified price vector, width-sized.
    pub fn active_prices(&self) -> &[u64] {
        self.prices.get(..self.width()).unwrap_or(&[])
    }
}

/// Decode one canonical certificate and check every structural fact the record
/// alone can carry.
///
/// The check order mirrors Lean's `PhysicalAbi.decodeChecks` position for
/// position: length, magic, schema, profile, reserved, scale, mass, degree,
/// width, atom count, padding, denominators, weights, coordinate order, weight
/// mass, primitive scale, price partition. The three checks that need a *basis*
/// — the binding, the coordinate admission and the hull equation — belong to
/// [`verify_price_gate_v1`] instead.
pub fn decode_price_gate_v1(input: &[u8]) -> Result<PriceGateCertificateV1> {
    if input.len() != PRICE_GATE_REQUEST_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    if read_bytes::<8>(input, PRICE_GATE_MAGIC_OFFSET_V1)? != PRICE_GATE_MAGIC_V1 {
        return Err(Error::InvalidMagic);
    }
    if read_u16(input, PRICE_GATE_VERSION_OFFSET_V1)? != PRICE_GATE_SCHEMA_VERSION_V1 {
        return Err(Error::UnsupportedSchema);
    }
    if read_u16(input, PRICE_GATE_PROFILE_OFFSET_V1)? != PRICE_GATE_PROFILE_V1 {
        return Err(Error::PriceGateUnsupportedProfile);
    }
    let reserved_end = PRICE_GATE_RESERVED_OFFSET_V1
        .checked_add(PRICE_GATE_RESERVED_BYTES_V1)
        .ok_or(Error::InvalidLength)?;
    if input
        .get(PRICE_GATE_RESERVED_OFFSET_V1..reserved_end)
        .ok_or(Error::InvalidLength)?
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
        return Err(Error::PriceGateZeroMass);
    }
    let degree = read_u8(input, PRICE_GATE_DEGREE_OFFSET_V1)?;
    if !(crate::payoff::runtime_v3::BASIS_SPLINE_MINIMUM_DEGREE_V3
        ..=crate::payoff::runtime_v3::BASIS_SPLINE_MAXIMUM_DEGREE_V3)
        .contains(&degree)
    {
        return Err(Error::SplineDegreeOutOfProfile);
    }
    let width_byte = read_u8(input, PRICE_GATE_WIDTH_OFFSET_V1)?;
    let width = usize::from(width_byte);
    if width <= usize::from(degree) || width > PRICE_GATE_MAX_WIDTH_V1 {
        return Err(Error::PriceGateWidthOutOfRange);
    }
    let atom_byte = read_u8(input, PRICE_GATE_ATOM_COUNT_OFFSET_V1)?;
    let atom_count = usize::from(atom_byte);
    if atom_count == 0 || atom_count > PRICE_GATE_MAX_ATOMS_V1 {
        return Err(Error::PriceGateCapacity);
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
        *weights.get_mut(slot).ok_or(Error::InvalidLength)? = weight;
        *numerators.get_mut(slot).ok_or(Error::InvalidLength)? = numerator;
        *denominators.get_mut(slot).ok_or(Error::InvalidLength)? = denominator;
    }

    // Canonical padding: one certificate has exactly one encoding. Without this
    // a caller could park arbitrary bytes past the declared widths and change
    // the record's digest without changing what it asserts.
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
        return Err(Error::PriceGateNonCanonicalPadding);
    }
    if denominators
        .iter()
        .take(atom_count)
        .any(|denominator| *denominator == 0)
    {
        return Err(Error::ZeroDenominator);
    }
    if weights.iter().take(atom_count).any(|weight| *weight == 0) {
        return Err(Error::PriceGateZeroAtomWeight);
    }

    // Strictly increasing rational coordinates, by cross multiplication over
    // positive denominators. Forbidding a repeated coordinate is what gives one
    // support one encoding.
    for slot in 1..atom_count {
        let previous = slot
            .checked_sub(1)
            .ok_or(Error::PriceGateNonCanonicalAtomOrder)?;
        let left = i128::from(*numerators.get(previous).ok_or(Error::InvalidLength)?)
            .checked_mul(i128::from(
                *denominators.get(slot).ok_or(Error::InvalidLength)?,
            ))
            .ok_or(Error::ArithmeticOverflow)?;
        let right = i128::from(*numerators.get(slot).ok_or(Error::InvalidLength)?)
            .checked_mul(i128::from(
                *denominators.get(previous).ok_or(Error::InvalidLength)?,
            ))
            .ok_or(Error::ArithmeticOverflow)?;
        if left >= right {
            return Err(Error::PriceGateNonCanonicalAtomOrder);
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
        return Err(Error::PriceGateWeightMassMismatch);
    }
    // Scaling the weights and the mass together leaves the hull equation
    // unchanged, so the boundary admits only the primitive representative.
    // This canonicalizes the scale; it does NOT make the support unique.
    if divisor != 1 {
        return Err(Error::PriceGateNonPrimitiveWeightScale);
    }

    let mut price_sum = 0_u128;
    for price in prices.iter().take(width) {
        price_sum = price_sum
            .checked_add(u128::from(*price))
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != u128::from(scale) {
        return Err(Error::PriceGatePriceNotPartition);
    }

    Ok(PriceGateCertificateV1 {
        scale,
        mass,
        degree,
        width: width_byte,
        atom_count: atom_byte,
        prices,
        weights,
        numerators,
        denominators,
    })
}

/// Verify one certificate against one **already authenticated** basis.
///
/// This function does not authenticate the basis, and a certificate verified
/// against an attacker-chosen basis certifies nothing about a Market. The
/// caller's obligation is that `knots`, `knot_denominator`, `payout_scale`,
/// `degree` and `width` all came off a record whose digest was checked.
///
/// Refuses [`Error::PriceGateBasisMismatch`] when the certificate was issued
/// against different founding-fixed quantities than the basis carries, and
/// [`Error::PriceGateHullRefused`] when the hull identity fails at any claim.
pub fn verify_price_gate_v1<K: SplineKnotsV3 + ?Sized>(
    knots: &K,
    knot_denominator: u64,
    payout_scale: u64,
    degree: u8,
    width: u32,
    input: &[u8],
) -> Result<PriceGateCertificateV1> {
    let certificate = decode_price_gate_v1(input)?;
    let basis_width = usize::try_from(width).map_err(|_| Error::InvalidCount)?;
    if u64::from(certificate.scale) != payout_scale
        || certificate.degree != degree
        || certificate.width() != basis_width
    {
        return Err(Error::PriceGateBasisMismatch);
    }

    let mut mixture = [0_u128; PRICE_GATE_MAX_WIDTH_V1];
    let mut payouts = [0_u64; PRICE_GATE_MAX_WIDTH_V1];
    for slot in 0..certificate.atom_count() {
        let numerator = *certificate
            .numerators
            .get(slot)
            .ok_or(Error::InvalidLength)?;
        let denominator = *certificate
            .denominators
            .get(slot)
            .ok_or(Error::InvalidLength)?;
        // **Recomputed, never read.** The certificate chose the coordinate; the
        // basis chooses the payout.
        let weights_at = evaluate_spline_weights_v3(
            knots,
            knot_denominator,
            i128::from(numerator),
            u64::from(denominator),
            degree,
            width,
        )?;
        let active = payouts
            .get_mut(..basis_width)
            .ok_or(Error::PriceGateBasisMismatch)?;
        apportion_cumulative_v3(&weights_at, payout_scale, active)?;

        let weight = u128::from(*certificate.weights.get(slot).ok_or(Error::InvalidLength)?);
        for claim in 0..basis_width {
            let payout = u128::from(*payouts.get(claim).ok_or(Error::InvalidLength)?);
            let weighted = weight
                .checked_mul(payout)
                .ok_or(Error::ArithmeticOverflow)?;
            let cell = mixture.get_mut(claim).ok_or(Error::PriceGateHullRefused)?;
            *cell = cell
                .checked_add(weighted)
                .ok_or(Error::ArithmeticOverflow)?;
        }
    }

    for claim in 0..certificate.width() {
        let scaled = u128::from(*certificate.prices.get(claim).ok_or(Error::InvalidLength)?)
            .checked_mul(u128::from(certificate.mass))
            .ok_or(Error::ArithmeticOverflow)?;
        if scaled != *mixture.get(claim).ok_or(Error::PriceGateHullRefused)? {
            return Err(Error::PriceGateHullRefused);
        }
    }
    Ok(certificate)
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left
}

fn slot_offset(base: usize, slot: usize, width: usize) -> Result<usize> {
    slot.checked_mul(width)
        .and_then(|shift| base.checked_add(shift))
        .ok_or(Error::InvalidLength)
}

fn read_bytes<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    input
        .get(offset..end)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(Error::InvalidLength)
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_bytes::<2>(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_bytes::<4>(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_bytes::<8>(input, offset)?))
}

fn read_i64(input: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(read_bytes::<8>(input, offset)?))
}
