#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact signed-rational successor to the bounded Product payoff interpreter.
//!
//! V2 evaluates every nonzero-denominator signed rational coordinate. Ramp and
//! tent tails clamp to their endpoint values, so the evaluator is total over
//! the canonical Source result line. The sole rounding boundary is the final
//! floor of a nonnegative rational interpolation into scaled payout units.
//! Parsing, evaluation, and the internal 256-bit arithmetic are safe,
//! fixed-memory, `no_std`, and allocation-free. Account and release authority
//! remain adapter obligations.

use core::{cmp::Ordering, convert::TryInto};

/// Registry-finalized graded-basis and projection-certificate admission.
pub mod registry_v3;
/// Data-defined runtime-width categorical and graded liability bases.
pub mod runtime_v3;

/// Canonical V2 wire magic.
pub const MAGIC_V2: [u8; 8] = *b"DCLTPAY2";
/// Canonical V2 wire version.
pub const VERSION_V2: u16 = 2;
/// Fixed physical-profile knot capacity.
pub const MAX_KNOTS_V2: usize = 16;
/// Fixed physical-profile term capacity.
pub const MAX_TERMS_V2: usize = 16;
/// Exact V2 ABI width.
pub const ABI_BYTES_V2: usize = 576;
/// Exact V2 header width.
pub const HEADER_BYTES_V2: usize = 64;
/// Width of one signed knot numerator.
pub const KNOT_BYTES_V2: usize = 16;
/// Width of one payoff term.
pub const TERM_BYTES_V2: usize = 16;
/// First signed knot-numerator offset.
pub const KNOTS_OFFSET_V2: usize = 64;
/// First term offset.
pub const TERMS_OFFSET_V2: usize = 320;

const KNOT_COUNT_OFFSET: usize = 10;
const TERM_COUNT_OFFSET: usize = 11;
const HEADER_RESERVED_OFFSET: usize = 12;
const PRODUCT_ID_OFFSET: usize = 16;
const DOMAIN_ID_OFFSET: usize = 24;
const COORDINATE_UNIT_ID_OFFSET: usize = 32;
const PAYOUT_SCALE_OFFSET: usize = 40;
const KNOT_DENOMINATOR_OFFSET: usize = 48;
const HEADER_TAIL_RESERVED_OFFSET: usize = 56;

/// Refusal from the V2 fixed-layout interpreter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have its sole exact width.
    InvalidLength,
    /// Magic selected a different wire family.
    InvalidMagic,
    /// The encoded version is unsupported.
    UnsupportedVersion,
    /// Reserved or inactive bytes were nonzero.
    NonCanonicalReserved,
    /// A count was zero, too small, or outside the physical profile.
    InvalidCount,
    /// A semantic scalar identity or rational denominator was zero.
    InvalidIdentity,
    /// Active signed knot numerators were not strictly increasing.
    NonCanonicalKnots,
    /// A term tag, index tuple, or knot reference was invalid.
    InvalidShape,
    /// A term amplitude was zero.
    ZeroAmplitude,
    /// Term shape keys were not strictly increasing.
    NonCanonicalTermOrder,
    /// The conservative sum-of-amplitudes bound overflowed `u64`.
    LiabilityOverflow,
    /// The supplied exact rational coordinate had a zero denominator.
    ZeroCoordinateDenominator,
    /// Exact fixed-width arithmetic overflowed or contradicted an interior case.
    ArithmeticOverflow,
}

/// Canonical finite payoff shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeV2 {
    /// Constant payout over the full signed-rational line.
    Constant,
    /// Increasing ramp with clamped endpoint tails.
    RampUp {
        /// Left Product-owned knot index.
        left: u8,
        /// Right Product-owned knot index.
        right: u8,
    },
    /// Decreasing ramp with clamped endpoint tails.
    RampDown {
        /// Left Product-owned knot index.
        left: u8,
        /// Right Product-owned knot index.
        right: u8,
    },
    /// Tent with zero outer tails and one Product-owned peak.
    Tent {
        /// Left Product-owned knot index.
        left: u8,
        /// Peak Product-owned knot index.
        peak: u8,
        /// Right Product-owned knot index.
        right: u8,
    },
}

/// One nonnegative scaled payout term.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TermV2 {
    /// Canonical shape.
    pub shape: ShapeV2,
    /// Nonzero scaled amplitude.
    pub amplitude: u64,
}

const EMPTY_TERM: TermV2 = TermV2 {
    shape: ShapeV2::Constant,
    amplitude: 0,
};

/// Decoded fixed-capacity signed-rational Product payoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductPayoffV2 {
    product_id: u64,
    domain_id: u64,
    coordinate_unit_id: u64,
    payout_scale: u64,
    knot_denominator: u64,
    knot_count: u8,
    term_count: u8,
    knots: [i128; MAX_KNOTS_V2],
    terms: [TermV2; MAX_TERMS_V2],
    liability_bound: u64,
}

impl ProductPayoffV2 {
    /// Hostile-decode one exact canonical V2 payoff record.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != ABI_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(MAGIC_V2.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, 8)? != VERSION_V2 {
            return Err(Error::UnsupportedVersion);
        }
        if !zero_span(input, HEADER_RESERVED_OFFSET, 4)?
            || !zero_span(input, HEADER_TAIL_RESERVED_OFFSET, 8)?
        {
            return Err(Error::NonCanonicalReserved);
        }
        let knot_count = read_byte(input, KNOT_COUNT_OFFSET)?;
        let term_count = read_byte(input, TERM_COUNT_OFFSET)?;
        if !(2..=MAX_KNOTS_V2).contains(&usize::from(knot_count))
            || !(1..=MAX_TERMS_V2).contains(&usize::from(term_count))
        {
            return Err(Error::InvalidCount);
        }
        let product_id = read_u64(input, PRODUCT_ID_OFFSET)?;
        let domain_id = read_u64(input, DOMAIN_ID_OFFSET)?;
        let coordinate_unit_id = read_u64(input, COORDINATE_UNIT_ID_OFFSET)?;
        let payout_scale = read_u64(input, PAYOUT_SCALE_OFFSET)?;
        let knot_denominator = read_u64(input, KNOT_DENOMINATOR_OFFSET)?;
        if product_id == 0
            || domain_id == 0
            || coordinate_unit_id == 0
            || payout_scale == 0
            || knot_denominator == 0
        {
            return Err(Error::InvalidIdentity);
        }

        let mut knots = [0_i128; MAX_KNOTS_V2];
        let mut prior = None;
        let mut knot_index = 0_usize;
        while knot_index < usize::from(knot_count) {
            let offset = element_offset(KNOTS_OFFSET_V2, knot_index, KNOT_BYTES_V2)?;
            let knot = read_i128(input, offset)?;
            if prior.is_some_and(|value| value >= knot) {
                return Err(Error::NonCanonicalKnots);
            }
            *knots.get_mut(knot_index).ok_or(Error::InvalidCount)? = knot;
            prior = Some(knot);
            knot_index = knot_index.checked_add(1).ok_or(Error::InvalidCount)?;
        }
        let inactive_knots = MAX_KNOTS_V2
            .checked_sub(usize::from(knot_count))
            .and_then(|count| count.checked_mul(KNOT_BYTES_V2))
            .ok_or(Error::InvalidLength)?;
        let inactive_knot_offset =
            element_offset(KNOTS_OFFSET_V2, usize::from(knot_count), KNOT_BYTES_V2)?;
        if !zero_span(input, inactive_knot_offset, inactive_knots)? {
            return Err(Error::NonCanonicalReserved);
        }

        let mut terms = [EMPTY_TERM; MAX_TERMS_V2];
        let mut liability_bound = 0_u64;
        let mut prior_key = None;
        let mut term_index = 0_usize;
        while term_index < usize::from(term_count) {
            let offset = element_offset(TERMS_OFFSET_V2, term_index, TERM_BYTES_V2)?;
            let term = decode_term(input, offset, knot_count)?;
            let key = shape_key(term.shape);
            if prior_key.is_some_and(|prior| prior >= key) {
                return Err(Error::NonCanonicalTermOrder);
            }
            liability_bound = liability_bound
                .checked_add(term.amplitude)
                .ok_or(Error::LiabilityOverflow)?;
            *terms.get_mut(term_index).ok_or(Error::InvalidCount)? = term;
            prior_key = Some(key);
            term_index = term_index.checked_add(1).ok_or(Error::InvalidCount)?;
        }
        let inactive_terms = MAX_TERMS_V2
            .checked_sub(usize::from(term_count))
            .and_then(|count| count.checked_mul(TERM_BYTES_V2))
            .ok_or(Error::InvalidLength)?;
        let inactive_term_offset =
            element_offset(TERMS_OFFSET_V2, usize::from(term_count), TERM_BYTES_V2)?;
        if !zero_span(input, inactive_term_offset, inactive_terms)? {
            return Err(Error::NonCanonicalReserved);
        }

        Ok(Self {
            product_id,
            domain_id,
            coordinate_unit_id,
            payout_scale,
            knot_denominator,
            knot_count,
            term_count,
            knots,
            terms,
            liability_bound,
        })
    }

    /// Encode the sole canonical V2 payoff record.
    pub fn to_bytes(self) -> [u8; ABI_BYTES_V2] {
        let mut output = [0_u8; ABI_BYTES_V2];
        put(&mut output, 0, &MAGIC_V2);
        put(&mut output, 8, &VERSION_V2.to_le_bytes());
        put_byte(&mut output, KNOT_COUNT_OFFSET, self.knot_count);
        put_byte(&mut output, TERM_COUNT_OFFSET, self.term_count);
        put(
            &mut output,
            PRODUCT_ID_OFFSET,
            &self.product_id.to_le_bytes(),
        );
        put(&mut output, DOMAIN_ID_OFFSET, &self.domain_id.to_le_bytes());
        put(
            &mut output,
            COORDINATE_UNIT_ID_OFFSET,
            &self.coordinate_unit_id.to_le_bytes(),
        );
        put(
            &mut output,
            PAYOUT_SCALE_OFFSET,
            &self.payout_scale.to_le_bytes(),
        );
        put(
            &mut output,
            KNOT_DENOMINATOR_OFFSET,
            &self.knot_denominator.to_le_bytes(),
        );
        let mut index = 0_usize;
        while index < usize::from(self.knot_count) {
            if let (Some(knot), Ok(offset)) = (
                self.knots.get(index),
                element_offset(KNOTS_OFFSET_V2, index, KNOT_BYTES_V2),
            ) {
                put(&mut output, offset, &knot.to_le_bytes());
            }
            index = index.saturating_add(1);
        }
        index = 0;
        while index < usize::from(self.term_count) {
            if let (Some(term), Ok(offset)) = (
                self.terms.get(index),
                element_offset(TERMS_OFFSET_V2, index, TERM_BYTES_V2),
            ) {
                encode_term(&mut output, offset, *term);
            }
            index = index.saturating_add(1);
        }
        output
    }

    /// Product semantic scalar identity.
    pub const fn product_id(self) -> u64 {
        self.product_id
    }

    /// Product-owned result-domain scalar identity.
    pub const fn domain_id(self) -> u64 {
        self.domain_id
    }

    /// Exact coordinate-unit scalar identity.
    pub const fn coordinate_unit_id(self) -> u64 {
        self.coordinate_unit_id
    }

    /// Exact payout unit scale.
    pub const fn payout_scale(self) -> u64 {
        self.payout_scale
    }

    /// Positive common denominator for every signed knot numerator.
    pub const fn knot_denominator(self) -> u64 {
        self.knot_denominator
    }

    /// Number of active canonical knots.
    pub const fn knot_count(self) -> u8 {
        self.knot_count
    }

    /// Return one active signed knot numerator.
    pub fn knot_numerator(self, index: usize) -> Option<i128> {
        if index < usize::from(self.knot_count) {
            self.knots.get(index).copied()
        } else {
            None
        }
    }

    /// Number of active canonical terms.
    pub const fn term_count(self) -> u8 {
        self.term_count
    }

    /// Return one active payoff term.
    pub fn term(self, index: usize) -> Option<TermV2> {
        if index < usize::from(self.term_count) {
            self.terms.get(index).copied()
        } else {
            None
        }
    }

    /// Conservative sum-of-amplitudes liability bound.
    pub const fn liability_bound(self) -> u64 {
        self.liability_bound
    }

    /// Whether available scaled collateral covers the conservative bound.
    pub const fn collateralized_by(self, available: u64) -> bool {
        self.liability_bound <= available
    }

    /// Evaluate one exact signed-rational coordinate.
    ///
    /// Outer coordinates are handled by each shape's explicit clamped tail.
    /// Interior interpolation performs no coordinate quantization: the final
    /// scaled payout floor is the sole rounding boundary.
    pub fn evaluate_rational(self, numerator: i128, denominator: u64) -> Result<u64, Error> {
        if denominator == 0 {
            return Err(Error::ZeroCoordinateDenominator);
        }
        let mut payout = 0_u64;
        let mut index = 0_usize;
        while index < usize::from(self.term_count) {
            let term = self.term(index).ok_or(Error::InvalidCount)?;
            let value = evaluate_term(self, term, numerator, denominator)?;
            payout = payout.checked_add(value).ok_or(Error::ArithmeticOverflow)?;
            index = index.checked_add(1).ok_or(Error::InvalidCount)?;
        }
        Ok(payout)
    }
}

fn evaluate_term(
    program: ProductPayoffV2,
    term: TermV2,
    numerator: i128,
    denominator: u64,
) -> Result<u64, Error> {
    match term.shape {
        ShapeV2::Constant => Ok(term.amplitude),
        ShapeV2::RampUp { left, right } => ramp_up(
            term.amplitude,
            program
                .knot_numerator(usize::from(left))
                .ok_or(Error::InvalidShape)?,
            program
                .knot_numerator(usize::from(right))
                .ok_or(Error::InvalidShape)?,
            program.knot_denominator,
            numerator,
            denominator,
        ),
        ShapeV2::RampDown { left, right } => ramp_down(
            term.amplitude,
            program
                .knot_numerator(usize::from(left))
                .ok_or(Error::InvalidShape)?,
            program
                .knot_numerator(usize::from(right))
                .ok_or(Error::InvalidShape)?,
            program.knot_denominator,
            numerator,
            denominator,
        ),
        ShapeV2::Tent { left, peak, right } => {
            let rising = ramp_up(
                term.amplitude,
                program
                    .knot_numerator(usize::from(left))
                    .ok_or(Error::InvalidShape)?,
                program
                    .knot_numerator(usize::from(peak))
                    .ok_or(Error::InvalidShape)?,
                program.knot_denominator,
                numerator,
                denominator,
            )?;
            let falling = ramp_down(
                term.amplitude,
                program
                    .knot_numerator(usize::from(peak))
                    .ok_or(Error::InvalidShape)?,
                program
                    .knot_numerator(usize::from(right))
                    .ok_or(Error::InvalidShape)?,
                program.knot_denominator,
                numerator,
                denominator,
            )?;
            Ok(rising.min(falling))
        }
    }
}

fn ramp_up(
    amplitude: u64,
    left: i128,
    right: i128,
    knot_denominator: u64,
    numerator: i128,
    denominator: u64,
) -> Result<u64, Error> {
    if rational_compare(numerator, denominator, left, knot_denominator)? != Ordering::Greater {
        Ok(0)
    } else if rational_compare(numerator, denominator, right, knot_denominator)? != Ordering::Less {
        Ok(amplitude)
    } else {
        interpolation_floor(
            amplitude,
            numerator,
            denominator,
            left,
            right,
            knot_denominator,
            true,
        )
    }
}

fn ramp_down(
    amplitude: u64,
    left: i128,
    right: i128,
    knot_denominator: u64,
    numerator: i128,
    denominator: u64,
) -> Result<u64, Error> {
    if rational_compare(numerator, denominator, left, knot_denominator)? != Ordering::Greater {
        Ok(amplitude)
    } else if rational_compare(numerator, denominator, right, knot_denominator)? != Ordering::Less {
        Ok(0)
    } else {
        interpolation_floor(
            amplitude,
            numerator,
            denominator,
            left,
            right,
            knot_denominator,
            false,
        )
    }
}

/// The sole V2 rounding boundary: floor one nonnegative exact rational
/// interpolation into scaled payout units.
fn interpolation_floor(
    amplitude: u64,
    numerator: i128,
    denominator: u64,
    left: i128,
    right: i128,
    knot_denominator: u64,
    rising: bool,
) -> Result<u64, Error> {
    let coordinate_scaled = SignedU256::product(numerator, knot_denominator)?;
    let left_scaled = SignedU256::product(left, denominator)?;
    let right_scaled = SignedU256::product(right, denominator)?;
    let elapsed = if rising {
        coordinate_scaled
            .subtract(left_scaled)?
            .positive_magnitude()?
    } else {
        right_scaled
            .subtract(coordinate_scaled)?
            .positive_magnitude()?
    };
    let width = right_scaled.subtract(left_scaled)?.positive_magnitude()?;
    if elapsed >= width || elapsed.is_zero() || width.is_zero() {
        return Err(Error::ArithmeticOverflow);
    }
    let scaled_elapsed = elapsed.checked_mul_u64(amplitude)?;

    // The quotient is known to lie in [0, amplitude]. Binary search avoids a
    // dynamic big-integer dependency and keeps this fixed-memory boundary total.
    let mut low = 0_u64;
    let mut high = amplitude;
    while low < high {
        let delta = high.checked_sub(low).ok_or(Error::ArithmeticOverflow)?;
        let half = delta / 2;
        let round_up = delta % 2;
        let middle = low
            .checked_add(half)
            .and_then(|value| value.checked_add(round_up))
            .ok_or(Error::ArithmeticOverflow)?;
        if width.checked_mul_u64(middle)? <= scaled_elapsed {
            low = middle;
        } else {
            high = middle.checked_sub(1).ok_or(Error::ArithmeticOverflow)?;
        }
    }
    Ok(low)
}

fn rational_compare(
    left_numerator: i128,
    left_denominator: u64,
    right_numerator: i128,
    right_denominator: u64,
) -> Result<Ordering, Error> {
    if left_denominator == 0 || right_denominator == 0 {
        return Err(Error::ZeroCoordinateDenominator);
    }
    let left = SignedU256::product(left_numerator, right_denominator)?;
    let right = SignedU256::product(right_numerator, left_denominator)?;
    Ok(left.cmp(&right))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignedU256 {
    negative: bool,
    magnitude: U256,
}

impl SignedU256 {
    fn product(value: i128, factor: u64) -> Result<Self, Error> {
        let magnitude = U256::from_u128(value.unsigned_abs()).checked_mul_u64(factor)?;
        Ok(Self {
            negative: value.is_negative() && !magnitude.is_zero(),
            magnitude,
        })
    }

    fn subtract(self, other: Self) -> Result<Self, Error> {
        match (self.negative, other.negative) {
            (false, false) => signed_magnitude_difference(self.magnitude, other.magnitude),
            (true, true) => signed_magnitude_difference(other.magnitude, self.magnitude),
            (false, true) => Ok(Self {
                negative: false,
                magnitude: self.magnitude.checked_add(other.magnitude)?,
            }),
            (true, false) => Ok(Self {
                negative: true,
                magnitude: self.magnitude.checked_add(other.magnitude)?,
            }),
        }
    }

    fn positive_magnitude(self) -> Result<U256, Error> {
        if self.negative || self.magnitude.is_zero() {
            Err(Error::ArithmeticOverflow)
        } else {
            Ok(self.magnitude)
        }
    }
}

impl Ord for SignedU256 {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.negative, other.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => self.magnitude.cmp(&other.magnitude),
            (true, true) => other.magnitude.cmp(&self.magnitude),
        }
    }
}

impl PartialOrd for SignedU256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn signed_magnitude_difference(left: U256, right: U256) -> Result<SignedU256, Error> {
    match left.cmp(&right) {
        Ordering::Equal => Ok(SignedU256 {
            negative: false,
            magnitude: U256::ZERO,
        }),
        Ordering::Greater => Ok(SignedU256 {
            negative: false,
            magnitude: left.checked_sub(right)?,
        }),
        Ordering::Less => Ok(SignedU256 {
            negative: true,
            magnitude: right.checked_sub(left)?,
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct U256([u64; 4]);

impl U256 {
    const ZERO: Self = Self([0; 4]);

    fn from_u128(value: u128) -> Self {
        let low = u64::try_from(value & u128::from(u64::MAX)).unwrap_or(0);
        let high = u64::try_from(value >> 64).unwrap_or(0);
        Self([low, high, 0, 0])
    }

    fn is_zero(self) -> bool {
        self.0.iter().all(|limb| *limb == 0)
    }

    fn checked_add(self, other: Self) -> Result<Self, Error> {
        let mut output = [0_u64; 4];
        let mut carry = 0_u128;
        let mut index = 0_usize;
        while index < output.len() {
            let left = u128::from(*self.0.get(index).ok_or(Error::ArithmeticOverflow)?);
            let right = u128::from(*other.0.get(index).ok_or(Error::ArithmeticOverflow)?);
            let sum = left
                .checked_add(right)
                .and_then(|value| value.checked_add(carry))
                .ok_or(Error::ArithmeticOverflow)?;
            *output.get_mut(index).ok_or(Error::ArithmeticOverflow)? = low_u64(sum)?;
            carry = sum >> 64;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if carry != 0 {
            return Err(Error::ArithmeticOverflow);
        }
        Ok(Self(output))
    }

    fn checked_sub(self, other: Self) -> Result<Self, Error> {
        if self < other {
            return Err(Error::ArithmeticOverflow);
        }
        let mut output = [0_u64; 4];
        let mut borrow = 0_u128;
        let base = u128::from(u64::MAX)
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut index = 0_usize;
        while index < output.len() {
            let left = u128::from(*self.0.get(index).ok_or(Error::ArithmeticOverflow)?);
            let right = u128::from(*other.0.get(index).ok_or(Error::ArithmeticOverflow)?)
                .checked_add(borrow)
                .ok_or(Error::ArithmeticOverflow)?;
            let (difference, next_borrow) = if left >= right {
                (left.checked_sub(right).ok_or(Error::ArithmeticOverflow)?, 0)
            } else {
                (
                    left.checked_add(base)
                        .and_then(|value| value.checked_sub(right))
                        .ok_or(Error::ArithmeticOverflow)?,
                    1,
                )
            };
            *output.get_mut(index).ok_or(Error::ArithmeticOverflow)? = low_u64(difference)?;
            borrow = next_borrow;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if borrow != 0 {
            return Err(Error::ArithmeticOverflow);
        }
        Ok(Self(output))
    }

    fn checked_mul_u64(self, multiplier: u64) -> Result<Self, Error> {
        let mut output = [0_u64; 4];
        let mut carry = 0_u128;
        let mut index = 0_usize;
        while index < output.len() {
            let limb = u128::from(*self.0.get(index).ok_or(Error::ArithmeticOverflow)?);
            let product = limb
                .checked_mul(u128::from(multiplier))
                .and_then(|value| value.checked_add(carry))
                .ok_or(Error::ArithmeticOverflow)?;
            *output.get_mut(index).ok_or(Error::ArithmeticOverflow)? = low_u64(product)?;
            carry = product >> 64;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if carry != 0 {
            return Err(Error::ArithmeticOverflow);
        }
        Ok(Self(output))
    }
}

impl Ord for U256 {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut index = self.0.len();
        while index != 0 {
            index = index.saturating_sub(1);
            let left = self.0.get(index).copied().unwrap_or(0);
            let right = other.0.get(index).copied().unwrap_or(0);
            match left.cmp(&right) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn low_u64(value: u128) -> Result<u64, Error> {
    u64::try_from(value & u128::from(u64::MAX)).map_err(|_| Error::ArithmeticOverflow)
}

fn decode_term(input: &[u8], offset: usize, knot_count: u8) -> Result<TermV2, Error> {
    if !zero_span(input, offset.checked_add(4).ok_or(Error::InvalidLength)?, 4)? {
        return Err(Error::NonCanonicalReserved);
    }
    let tag = read_byte(input, offset)?;
    let left = read_byte(input, offset.checked_add(1).ok_or(Error::InvalidLength)?)?;
    let peak = read_byte(input, offset.checked_add(2).ok_or(Error::InvalidLength)?)?;
    let right = read_byte(input, offset.checked_add(3).ok_or(Error::InvalidLength)?)?;
    let amplitude = read_u64(input, offset.checked_add(8).ok_or(Error::InvalidLength)?)?;
    if amplitude == 0 {
        return Err(Error::ZeroAmplitude);
    }
    let shape = match tag {
        0 if left == 0 && peak == 0 && right == 0 => ShapeV2::Constant,
        1 if peak == 0 && left < right && right < knot_count => ShapeV2::RampUp { left, right },
        2 if peak == 0 && left < right && right < knot_count => ShapeV2::RampDown { left, right },
        3 if left < peak && peak < right && right < knot_count => {
            ShapeV2::Tent { left, peak, right }
        }
        _ => return Err(Error::InvalidShape),
    };
    Ok(TermV2 { shape, amplitude })
}

fn encode_term(output: &mut [u8; ABI_BYTES_V2], offset: usize, term: TermV2) {
    let (tag, left, peak, right) = match term.shape {
        ShapeV2::Constant => (0, 0, 0, 0),
        ShapeV2::RampUp { left, right } => (1, left, 0, right),
        ShapeV2::RampDown { left, right } => (2, left, 0, right),
        ShapeV2::Tent { left, peak, right } => (3, left, peak, right),
    };
    put_byte(output, offset, tag);
    put_byte(output, offset.saturating_add(1), left);
    put_byte(output, offset.saturating_add(2), peak);
    put_byte(output, offset.saturating_add(3), right);
    put(
        output,
        offset.saturating_add(8),
        &term.amplitude.to_le_bytes(),
    );
}

fn shape_key(shape: ShapeV2) -> u64 {
    match shape {
        ShapeV2::Constant => 0,
        ShapeV2::RampUp { left, right } => 4096 + u64::from(left) * 16 + u64::from(right),
        ShapeV2::RampDown { left, right } => 8192 + u64::from(left) * 16 + u64::from(right),
        ShapeV2::Tent { left, peak, right } => {
            12288 + u64::from(left) * 256 + u64::from(peak) * 16 + u64::from(right)
        }
    }
}

fn element_offset(base: usize, index: usize, width: usize) -> Result<usize, Error> {
    index
        .checked_mul(width)
        .and_then(|relative| base.checked_add(relative))
        .ok_or(Error::InvalidLength)
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_i128(input: &[u8], offset: usize) -> Result<i128, Error> {
    Ok(i128::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn zero_span(input: &[u8], offset: usize, width: usize) -> Result<bool, Error> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    Ok(input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
    if let Some(end) = offset.checked_add(bytes.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(bytes);
    }
}

fn put_byte(output: &mut [u8], offset: usize, byte: u8) {
    if let Some(destination) = output.get_mut(offset) {
        *destination = byte;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    fn fixture_bytes() -> [u8; ABI_BYTES_V2] {
        let mut bytes = [0_u8; ABI_BYTES_V2];
        put(&mut bytes, 0, &MAGIC_V2);
        put(&mut bytes, 8, &VERSION_V2.to_le_bytes());
        put_byte(&mut bytes, KNOT_COUNT_OFFSET, 5);
        put_byte(&mut bytes, TERM_COUNT_OFFSET, 4);
        for (offset, value) in [
            (PRODUCT_ID_OFFSET, 8101_u64),
            (DOMAIN_ID_OFFSET, 7001),
            (COORDINATE_UNIT_ID_OFFSET, 9),
            (PAYOUT_SCALE_OFFSET, 100),
            (KNOT_DENOMINATOR_OFFSET, 2),
        ] {
            put(&mut bytes, offset, &value.to_le_bytes());
        }
        for (index, knot) in [-100_i128, -50, 0, 50, 100].into_iter().enumerate() {
            let offset = element_offset(KNOTS_OFFSET_V2, index, KNOT_BYTES_V2).expect("offset");
            put(&mut bytes, offset, &knot.to_le_bytes());
        }
        for (index, (tag, left, peak, right, amplitude)) in [
            (0_u8, 0_u8, 0_u8, 0_u8, 2_u64),
            (1, 0, 0, 4, 10),
            (2, 0, 0, 4, 5),
            (3, 1, 2, 3, 20),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = element_offset(TERMS_OFFSET_V2, index, TERM_BYTES_V2).expect("offset");
            put_byte(&mut bytes, offset, tag);
            put_byte(&mut bytes, offset + 1, left);
            put_byte(&mut bytes, offset + 2, peak);
            put_byte(&mut bytes, offset + 3, right);
            put(&mut bytes, offset + 8, &amplitude.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn exact_width_roundtrip_and_hostile_canonicality() {
        let bytes = fixture_bytes();
        let product = ProductPayoffV2::decode(&bytes).expect("canonical product");
        assert_eq!(product.to_bytes(), bytes);
        for width in 0..ABI_BYTES_V2 {
            assert_eq!(
                ProductPayoffV2::decode(bytes.get(..width).expect("bounded width")),
                Err(Error::InvalidLength)
            );
        }
        let mut padded = Vec::from(bytes);
        padded.push(0);
        assert_eq!(ProductPayoffV2::decode(&padded), Err(Error::InvalidLength));
        for offset in [0, 8, 12, 56, 144, 324, 400] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("hostile offset") ^= 1;
            assert!(
                ProductPayoffV2::decode(&hostile).is_err(),
                "offset {offset}"
            );
        }
    }

    #[test]
    fn signed_rational_line_is_total_with_explicit_clamped_tails() {
        let product = ProductPayoffV2::decode(&fixture_bytes()).expect("product");
        let cases = [
            (i128::MIN, u64::MAX, 7_u64),
            (-1000, 1, 7),
            (-50, 2, 7),
            (0, 1, 29),
            (75, 2, 10),
            (1000, 1, 12),
            (i128::MAX, u64::MAX, 12),
        ];
        for (numerator, denominator, expected) in cases {
            assert_eq!(
                product.evaluate_rational(numerator, denominator),
                Ok(expected),
                "{numerator}/{denominator}"
            );
        }
        assert_eq!(
            product.evaluate_rational(1, 0),
            Err(Error::ZeroCoordinateDenominator)
        );
    }

    #[test]
    fn interpolation_floors_once_without_coordinate_quantization() {
        let product = ProductPayoffV2::decode(&fixture_bytes()).expect("product");
        // 75/2 = 37.5. Ramp-up contributes floor(10 * 87.5 / 100) = 8,
        // ramp-down contributes 1, the tent is outside, and constant is 2.
        assert_eq!(product.evaluate_rational(75, 2), Ok(10));
        // Equivalent rationals are indistinguishable.
        assert_eq!(product.evaluate_rational(150, 4), Ok(10));
        // The old integral-coordinate surrogate would have queried 37 and
        // produced the same value here only accidentally; 99/2 exposes the
        // exact rational floor boundary.
        assert_eq!(product.evaluate_rational(99, 2), Ok(11));
    }

    #[test]
    fn extreme_cross_products_do_not_overflow() {
        assert_eq!(
            rational_compare(i128::MIN, u64::MAX, i128::MAX, u64::MAX),
            Ok(Ordering::Less)
        );
        assert_eq!(
            rational_compare(i128::MAX, 1, i128::MAX - 1, u64::MAX),
            Ok(Ordering::Greater)
        );
        let product = ProductPayoffV2::decode(&fixture_bytes()).expect("product");
        assert!(product.evaluate_rational(i128::MIN, 1).is_ok());
        assert!(product.evaluate_rational(i128::MAX, 1).is_ok());
    }

    #[test]
    fn liability_bound_covers_dense_exact_rational_corpus() {
        let product = ProductPayoffV2::decode(&fixture_bytes()).expect("product");
        let mut accepted = 0_u64;
        for denominator in 1_u64..=17 {
            for numerator in -400_i128..=400 {
                let payout = product
                    .evaluate_rational(numerator, denominator)
                    .expect("total rational evaluation");
                assert!(payout <= product.liability_bound());
                accepted = accepted.saturating_add(1);
            }
        }
        assert_eq!(accepted, 13_617);
        assert_eq!(product.liability_bound(), 37);
    }
}
