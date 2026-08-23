//! Fixed-width exact arithmetic for small fraction-free linear systems.
//!
//! The support-four inverse constructor needs exact 3-by-3 determinants of
//! signed differences between full-width `u64` payout atoms. This module keeps
//! that arithmetic separate from search policy. Its fixed magnitude has 2048
//! bits: enough for full-`u64` Bareiss intermediates through a 15-by-15 system,
//! the largest system induced by the 16-outcome profile, and the later
//! determinant-times-atom reconstruction bound. Every operation is checked,
//! and exact division refuses a remainder.

use core::cmp::Ordering;

const WIDE_LIMBS_V1: usize = 32;
const WIDE_BITS_V1: u32 = 2_048;
const MATRIX_SIDE_V1: usize = 3;
const MATRIX_CELLS_V1: usize = MATRIX_SIDE_V1 * MATRIX_SIDE_V1;
pub(crate) const MAX_FRACTION_FREE_SIDE_V1: usize = 15;
pub(crate) const MAX_FRACTION_FREE_ROWS_V1: usize = 16;
const MAX_FRACTION_FREE_CELLS_V1: usize =
    MAX_FRACTION_FREE_SIDE_V1 * MAX_FRACTION_FREE_SIDE_V1;
const MAX_RECTANGULAR_CELLS_V1: usize =
    MAX_FRACTION_FREE_ROWS_V1 * MAX_FRACTION_FREE_SIDE_V1;

/// Checked-arithmetic refusal from the fixed fraction-free substrate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FractionFreeErrorV1 {
    /// A result exceeded the fixed 2048-bit magnitude.
    ArithmeticOverflow,
    /// A fraction-free division unexpectedly had a remainder.
    NonExactDivision,
    /// An internal index or zero-divisor invariant was violated.
    InvariantViolation,
}

/// Result alias for the fixed fraction-free substrate.
pub(crate) type ResultFractionFreeV1<T> = core::result::Result<T, FractionFreeErrorV1>;

/// Unsigned 2048-bit magnitude in little-endian `u64` limbs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WideUnsignedV1 {
    limbs: [u64; WIDE_LIMBS_V1],
}

impl Ord for WideUnsignedV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut limb = WIDE_LIMBS_V1;
        while limb != 0 {
            limb -= 1;
            match self.limbs[limb].cmp(&other.limbs[limb]) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for WideUnsignedV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl WideUnsignedV1 {
    pub(crate) const ZERO: Self = Self {
        limbs: [0; WIDE_LIMBS_V1],
    };
    const ONE: Self = Self::from_u64(1);

    pub(crate) const fn from_u64(value: u64) -> Self {
        let mut limbs = [0; WIDE_LIMBS_V1];
        limbs[0] = value;
        Self { limbs }
    }

    pub(crate) fn from_u128(value: u128) -> ResultFractionFreeV1<Self> {
        let mut limbs = [0; WIDE_LIMBS_V1];
        limbs[0] = u64::try_from(value & u128::from(u64::MAX))
            .map_err(|_| FractionFreeErrorV1::InvariantViolation)?;
        limbs[1] = u64::try_from(value >> 64)
            .map_err(|_| FractionFreeErrorV1::InvariantViolation)?;
        Ok(Self { limbs })
    }

    pub(crate) const fn is_zero(self) -> bool {
        let mut limb = 0usize;
        while limb < WIDE_LIMBS_V1 {
            if self.limbs[limb] != 0 {
                return false;
            }
            limb += 1;
        }
        true
    }

    pub(crate) const fn limb(self, index: usize) -> Option<u64> {
        if index < WIDE_LIMBS_V1 {
            Some(self.limbs[index])
        } else {
            None
        }
    }

    pub(crate) fn checked_add(self, other: Self) -> Option<Self> {
        let mut limbs = [0u64; WIDE_LIMBS_V1];
        let mut carry = false;
        let mut limb = 0usize;
        while limb < WIDE_LIMBS_V1 {
            let (partial, first_carry) = self.limbs[limb].overflowing_add(other.limbs[limb]);
            let (sum, second_carry) = partial.overflowing_add(if carry { 1 } else { 0 });
            limbs[limb] = sum;
            carry = first_carry || second_carry;
            limb += 1;
        }
        if carry {
            None
        } else {
            Some(Self { limbs })
        }
    }

    pub(crate) fn checked_sub(self, other: Self) -> Option<Self> {
        if self < other {
            return None;
        }
        let mut limbs = [0u64; WIDE_LIMBS_V1];
        let mut borrow = false;
        let mut limb = 0usize;
        while limb < WIDE_LIMBS_V1 {
            let (partial, first_borrow) = self.limbs[limb].overflowing_sub(other.limbs[limb]);
            let (difference, second_borrow) =
                partial.overflowing_sub(if borrow { 1 } else { 0 });
            limbs[limb] = difference;
            borrow = first_borrow || second_borrow;
            limb += 1;
        }
        if borrow {
            None
        } else {
            Some(Self { limbs })
        }
    }

    pub(crate) fn checked_mul(self, other: Self) -> Option<Self> {
        let mut product = Self::ZERO;
        let left_length = self.significant_limbs();
        let right_length = other.significant_limbs();
        let mut left_limb = 0usize;
        while left_limb < left_length {
            let mut right_limb = 0usize;
            while right_limb < right_length {
                let factor = u128::from(self.limbs[left_limb])
                    .checked_mul(u128::from(other.limbs[right_limb]))?;
                if factor != 0 {
                    let target = left_limb.checked_add(right_limb)?;
                    if target >= WIDE_LIMBS_V1 {
                        return None;
                    }
                    let low = u64::try_from(factor & u128::from(u64::MAX)).ok()?;
                    let high = u64::try_from(factor >> 64).ok()?;
                    product.checked_add_word(target, low)?;
                    if high != 0 {
                        product.checked_add_word(target.checked_add(1)?, high)?;
                    }
                }
                right_limb += 1;
            }
            left_limb += 1;
        }
        Some(product)
    }

    pub(crate) fn checked_mul_u64(self, multiplier: u64) -> Option<Self> {
        self.checked_mul(Self::from_u64(multiplier))
    }

    fn checked_add_word(&mut self, mut index: usize, mut word: u64) -> Option<()> {
        while word != 0 {
            if index >= WIDE_LIMBS_V1 {
                return None;
            }
            let (sum, carry) = self.limbs[index].overflowing_add(word);
            self.limbs[index] = sum;
            word = if carry { 1 } else { 0 };
            index = index.checked_add(1)?;
        }
        Some(())
    }

    fn checked_shl(self, shift: u32) -> Option<Self> {
        if shift == 0 {
            return Some(self);
        }
        if shift >= WIDE_BITS_V1 {
            return if self.is_zero() { Some(Self::ZERO) } else { None };
        }
        let word_shift = usize::try_from(shift / 64).ok()?;
        let bit_shift = shift % 64;
        let mut shifted = Self::ZERO;
        let mut source = 0usize;
        while source < WIDE_LIMBS_V1 {
            let value = self.limbs[source];
            if value != 0 {
                let target = source.checked_add(word_shift)?;
                if target >= WIDE_LIMBS_V1 {
                    return None;
                }
                shifted.limbs[target] |= value << bit_shift;
                if bit_shift != 0 {
                    let carry = value >> (64 - bit_shift);
                    if carry != 0 {
                        let carry_target = target.checked_add(1)?;
                        if carry_target >= WIDE_LIMBS_V1 {
                            return None;
                        }
                        shifted.limbs[carry_target] |= carry;
                    }
                }
            }
            source += 1;
        }
        Some(shifted)
    }

    fn shr(self, shift: u32) -> Self {
        if shift == 0 {
            return self;
        }
        if shift >= WIDE_BITS_V1 {
            return Self::ZERO;
        }
        let word_shift = usize::try_from(shift / 64).unwrap_or(WIDE_LIMBS_V1);
        let bit_shift = shift % 64;
        let mut shifted = Self::ZERO;
        let mut target = 0usize;
        while target < WIDE_LIMBS_V1 {
            let source = target + word_shift;
            if source >= WIDE_LIMBS_V1 {
                break;
            }
            shifted.limbs[target] |= self.limbs[source] >> bit_shift;
            if bit_shift != 0 && source + 1 < WIDE_LIMBS_V1 {
                shifted.limbs[target] |= self.limbs[source + 1] << (64 - bit_shift);
            }
            target += 1;
        }
        shifted
    }

    fn trailing_zeros(self) -> ResultFractionFreeV1<u32> {
        let mut limb = 0usize;
        while limb < WIDE_LIMBS_V1 {
            if self.limbs[limb] != 0 {
                let limb_bits = u32::try_from(limb)
                    .map_err(|_| FractionFreeErrorV1::ArithmeticOverflow)?
                    .checked_mul(64)
                    .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
                return limb_bits
                    .checked_add(self.limbs[limb].trailing_zeros())
                    .ok_or(FractionFreeErrorV1::ArithmeticOverflow);
            }
            limb += 1;
        }
        Err(FractionFreeErrorV1::InvariantViolation)
    }

    fn bit(self, index: u32) -> bool {
        if index >= WIDE_BITS_V1 {
            return false;
        }
        let limb = usize::try_from(index / 64).unwrap_or(WIDE_LIMBS_V1);
        let bit = index % 64;
        limb < WIDE_LIMBS_V1 && ((self.limbs[limb] >> bit) & 1) != 0
    }

    fn set_bit(&mut self, index: u32) -> ResultFractionFreeV1<()> {
        if index >= WIDE_BITS_V1 {
            return Err(FractionFreeErrorV1::InvariantViolation);
        }
        let limb = usize::try_from(index / 64)
            .map_err(|_| FractionFreeErrorV1::ArithmeticOverflow)?;
        self.limbs[limb] |= 1u64 << (index % 64);
        Ok(())
    }

    pub(crate) fn checked_div_exact(self, divisor: Self) -> ResultFractionFreeV1<Self> {
        if divisor.is_zero() {
            return Err(FractionFreeErrorV1::InvariantViolation);
        }
        if let Some(small) = divisor.to_u64() {
            return self.checked_div_exact_u64(small);
        }
        let mut quotient = Self::ZERO;
        let mut remainder = Self::ZERO;
        let bit_length = self.bit_length()?;
        let mut cursor = u16::try_from(bit_length)
            .map_err(|_| FractionFreeErrorV1::ArithmeticOverflow)?;
        while cursor != 0 {
            cursor -= 1;
            remainder = remainder
                .checked_shl(1)
                .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
            if self.bit(u32::from(cursor)) {
                remainder = remainder
                    .checked_add(Self::ONE)
                    .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
            }
            if remainder >= divisor {
                remainder = remainder
                    .checked_sub(divisor)
                    .ok_or(FractionFreeErrorV1::InvariantViolation)?;
                quotient.set_bit(u32::from(cursor))?;
            }
        }
        if !remainder.is_zero() {
            return Err(FractionFreeErrorV1::NonExactDivision);
        }
        Ok(quotient)
    }

    fn checked_div_exact_u64(self, divisor: u64) -> ResultFractionFreeV1<Self> {
        if divisor == 0 {
            return Err(FractionFreeErrorV1::InvariantViolation);
        }
        let mut quotient = Self::ZERO;
        let mut remainder = 0u64;
        let mut limb = WIDE_LIMBS_V1;
        while limb != 0 {
            limb -= 1;
            let dividend = (u128::from(remainder) << 64) | u128::from(self.limbs[limb]);
            quotient.limbs[limb] = u64::try_from(dividend / u128::from(divisor))
                .map_err(|_| FractionFreeErrorV1::InvariantViolation)?;
            remainder = u64::try_from(dividend % u128::from(divisor))
                .map_err(|_| FractionFreeErrorV1::InvariantViolation)?;
        }
        if remainder != 0 {
            return Err(FractionFreeErrorV1::NonExactDivision);
        }
        Ok(quotient)
    }

    pub(crate) fn to_u64(self) -> Option<u64> {
        let mut limb = 1usize;
        while limb < WIDE_LIMBS_V1 {
            if self.limbs[limb] != 0 {
                return None;
            }
            limb += 1;
        }
        Some(self.limbs[0])
    }

    fn significant_limbs(self) -> usize {
        let mut limb = WIDE_LIMBS_V1;
        while limb != 0 {
            if self.limbs[limb - 1] != 0 {
                return limb;
            }
            limb -= 1;
        }
        0
    }

    fn bit_length(self) -> ResultFractionFreeV1<u32> {
        let significant = self.significant_limbs();
        if significant == 0 {
            return Ok(0);
        }
        let high = self.limbs[significant - 1];
        let lower_bits = u32::try_from(significant - 1)
            .map_err(|_| FractionFreeErrorV1::ArithmeticOverflow)?
            .checked_mul(64)
            .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
        lower_bits
            .checked_add(64 - high.leading_zeros())
            .ok_or(FractionFreeErrorV1::ArithmeticOverflow)
    }
}

/// A signed full-width input scalar formed without signed narrowing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SignedDeltaV1 {
    pub(crate) negative: bool,
    pub(crate) magnitude: u64,
}

impl SignedDeltaV1 {
    pub(crate) const fn between(left: u64, right: u64) -> Self {
        if left >= right {
            Self {
                negative: false,
                magnitude: left - right,
            }
        } else {
            Self {
                negative: true,
                magnitude: right - left,
            }
        }
    }

    pub(crate) fn product(self, other: Self) -> ResultFractionFreeV1<SignedWideV1> {
        let magnitude = u128::from(self.magnitude)
            .checked_mul(u128::from(other.magnitude))
            .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
        Ok(SignedWideV1::new(
            self.negative != other.negative,
            WideUnsignedV1::from_u128(magnitude)?,
        ))
    }

    const fn widened(self) -> SignedWideV1 {
        SignedWideV1::new(self.negative, WideUnsignedV1::from_u64(self.magnitude))
    }
}

/// Signed 2048-bit value with canonical positive zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SignedWideV1 {
    pub(crate) negative: bool,
    pub(crate) magnitude: WideUnsignedV1,
}

impl SignedWideV1 {
    const ZERO: Self = Self::new(false, WideUnsignedV1::ZERO);

    pub(crate) const fn new(negative: bool, magnitude: WideUnsignedV1) -> Self {
        Self {
            negative: negative && !magnitude.is_zero(),
            magnitude,
        }
    }

    pub(crate) fn checked_sub(self, other: Self) -> ResultFractionFreeV1<Self> {
        if self.negative == other.negative {
            if self.magnitude >= other.magnitude {
                Ok(Self::new(
                    self.negative,
                    self.magnitude
                        .checked_sub(other.magnitude)
                        .ok_or(FractionFreeErrorV1::InvariantViolation)?,
                ))
            } else {
                Ok(Self::new(
                    !self.negative,
                    other
                        .magnitude
                        .checked_sub(self.magnitude)
                        .ok_or(FractionFreeErrorV1::InvariantViolation)?,
                ))
            }
        } else {
            Ok(Self::new(
                self.negative,
                self.magnitude
                    .checked_add(other.magnitude)
                    .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?,
            ))
        }
    }

    fn checked_mul(self, other: Self) -> ResultFractionFreeV1<Self> {
        Ok(Self::new(
            self.negative != other.negative,
            self.magnitude
                .checked_mul(other.magnitude)
                .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?,
        ))
    }

    fn checked_div_exact(self, divisor: Self) -> ResultFractionFreeV1<Self> {
        Ok(Self::new(
            self.negative != divisor.negative,
            self.magnitude.checked_div_exact(divisor.magnitude)?,
        ))
    }

    pub(crate) const fn negated(self) -> Self {
        Self::new(!self.negative, self.magnitude)
    }
}

/// Exact fixed 3-by-3 matrix over full-width signed payout differences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FractionFreeMatrix3V1 {
    cells: [SignedWideV1; MATRIX_CELLS_V1],
}

impl FractionFreeMatrix3V1 {
    pub(crate) fn new(rows: [[SignedDeltaV1; MATRIX_SIDE_V1]; MATRIX_SIDE_V1]) -> Self {
        let mut cells = [SignedWideV1::ZERO; MATRIX_CELLS_V1];
        let mut row = 0usize;
        while row < MATRIX_SIDE_V1 {
            let mut column = 0usize;
            while column < MATRIX_SIDE_V1 {
                cells[row * MATRIX_SIDE_V1 + column] = rows[row][column].widened();
                column += 1;
            }
            row += 1;
        }
        Self { cells }
    }

    pub(crate) fn with_column(
        mut self,
        column: usize,
        values: [SignedDeltaV1; MATRIX_SIDE_V1],
    ) -> ResultFractionFreeV1<Self> {
        if column >= MATRIX_SIDE_V1 {
            return Err(FractionFreeErrorV1::InvariantViolation);
        }
        let mut row = 0usize;
        while row < MATRIX_SIDE_V1 {
            self.cells[row * MATRIX_SIDE_V1 + column] = values[row].widened();
            row += 1;
        }
        Ok(self)
    }

    /// Compute the signed determinant with row-pivoted Bareiss elimination.
    pub(crate) fn determinant(mut self) -> ResultFractionFreeV1<SignedWideV1> {
        let mut previous_pivot = SignedWideV1::new(false, WideUnsignedV1::from_u64(1));
        let mut sign_negative = false;
        let mut pivot_column = 0usize;
        while pivot_column + 1 < MATRIX_SIDE_V1 {
            let mut pivot_row = pivot_column;
            while pivot_row < MATRIX_SIDE_V1
                && self.get(pivot_row, pivot_column)?.magnitude.is_zero()
            {
                pivot_row += 1;
            }
            if pivot_row == MATRIX_SIDE_V1 {
                return Ok(SignedWideV1::ZERO);
            }
            if pivot_row != pivot_column {
                self.swap_rows(pivot_row, pivot_column)?;
                sign_negative = !sign_negative;
            }
            let pivot = self.get(pivot_column, pivot_column)?;
            let mut row = pivot_column + 1;
            while row < MATRIX_SIDE_V1 {
                let below = self.get(row, pivot_column)?;
                let mut column = pivot_column + 1;
                while column < MATRIX_SIDE_V1 {
                    let diagonal_product = pivot.checked_mul(self.get(row, column)?)?;
                    let cross_product = below.checked_mul(self.get(pivot_column, column)?)?;
                    let mut next = diagonal_product.checked_sub(cross_product)?;
                    if pivot_column != 0 {
                        next = next.checked_div_exact(previous_pivot)?;
                    }
                    self.set(row, column, next)?;
                    column += 1;
                }
                self.set(row, pivot_column, SignedWideV1::ZERO)?;
                row += 1;
            }
            previous_pivot = pivot;
            pivot_column += 1;
        }
        let determinant = self.get(MATRIX_SIDE_V1 - 1, MATRIX_SIDE_V1 - 1)?;
        Ok(if sign_negative {
            determinant.negated()
        } else {
            determinant
        })
    }

    fn get(self, row: usize, column: usize) -> ResultFractionFreeV1<SignedWideV1> {
        let index = row
            .checked_mul(MATRIX_SIDE_V1)
            .and_then(|value| value.checked_add(column))
            .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
        self.cells
            .get(index)
            .copied()
            .ok_or(FractionFreeErrorV1::InvariantViolation)
    }

    fn set(
        &mut self,
        row: usize,
        column: usize,
        value: SignedWideV1,
    ) -> ResultFractionFreeV1<()> {
        let index = row
            .checked_mul(MATRIX_SIDE_V1)
            .and_then(|base| base.checked_add(column))
            .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
        let cell = self
            .cells
            .get_mut(index)
            .ok_or(FractionFreeErrorV1::InvariantViolation)?;
        *cell = value;
        Ok(())
    }

    fn swap_rows(&mut self, left: usize, right: usize) -> ResultFractionFreeV1<()> {
        if left >= MATRIX_SIDE_V1 || right >= MATRIX_SIDE_V1 {
            return Err(FractionFreeErrorV1::InvariantViolation);
        }
        let mut column = 0usize;
        while column < MATRIX_SIDE_V1 {
            self.cells
                .swap(left * MATRIX_SIDE_V1 + column, right * MATRIX_SIDE_V1 + column);
            column += 1;
        }
        Ok(())
    }
}

/// Compact source matrix for one exact square system of side at most 15.
///
/// Source cells retain signed `u64` magnitudes. [`Self::determinant`] widens a
/// single working copy to the fixed 2048-bit profile, avoiding a persisted
/// wide matrix in callers that need several Cramer determinants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FractionFreeMatrixV1 {
    side: u8,
    cells: [SignedDeltaV1; MAX_FRACTION_FREE_CELLS_V1],
}

impl FractionFreeMatrixV1 {
    pub(crate) fn new(side: u8) -> ResultFractionFreeV1<Self> {
        let active = usize::from(side);
        if active == 0 || active > MAX_FRACTION_FREE_SIDE_V1 {
            return Err(FractionFreeErrorV1::InvariantViolation);
        }
        Ok(Self {
            side,
            cells: [SignedDeltaV1::between(0, 0); MAX_FRACTION_FREE_CELLS_V1],
        })
    }

    pub(crate) fn set(
        &mut self,
        row: usize,
        column: usize,
        value: SignedDeltaV1,
    ) -> ResultFractionFreeV1<()> {
        let index = self.index(row, column)?;
        self.cells[index] = value;
        Ok(())
    }

    pub(crate) fn with_column(
        mut self,
        column: usize,
        values: &[SignedDeltaV1; MAX_FRACTION_FREE_SIDE_V1],
    ) -> ResultFractionFreeV1<Self> {
        let side = usize::from(self.side);
        if column >= side {
            return Err(FractionFreeErrorV1::InvariantViolation);
        }
        let mut row = 0usize;
        while row < side {
            self.set(row, column, values[row])?;
            row += 1;
        }
        Ok(self)
    }

    /// Compute one exact determinant by row-pivoted Bareiss elimination.
    pub(crate) fn determinant(self) -> ResultFractionFreeV1<SignedWideV1> {
        let side = usize::from(self.side);
        let mut cells = [SignedWideV1::ZERO; MAX_FRACTION_FREE_CELLS_V1];
        let active_cells = side
            .checked_mul(side)
            .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
        let mut cell = 0usize;
        while cell < active_cells {
            cells[cell] = self.cells[cell].widened();
            cell += 1;
        }
        let mut previous_pivot = SignedWideV1::new(false, WideUnsignedV1::from_u64(1));
        let mut sign_negative = false;
        let mut pivot_column = 0usize;
        while pivot_column + 1 < side {
            let mut pivot_row = pivot_column;
            while pivot_row < side
                && wide_square_get(&cells, side, pivot_row, pivot_column)?
                    .magnitude
                    .is_zero()
            {
                pivot_row += 1;
            }
            if pivot_row == side {
                return Ok(SignedWideV1::ZERO);
            }
            if pivot_row != pivot_column {
                wide_square_swap_rows(&mut cells, side, pivot_row, pivot_column)?;
                sign_negative = !sign_negative;
            }
            let pivot = wide_square_get(&cells, side, pivot_column, pivot_column)?;
            let mut row = pivot_column + 1;
            while row < side {
                let below = wide_square_get(&cells, side, row, pivot_column)?;
                let mut column = pivot_column + 1;
                while column < side {
                    let diagonal_product = pivot
                        .checked_mul(wide_square_get(&cells, side, row, column)?)?;
                    let cross_product = below.checked_mul(wide_square_get(
                        &cells,
                        side,
                        pivot_column,
                        column,
                    )?)?;
                    let mut next = diagonal_product.checked_sub(cross_product)?;
                    if pivot_column != 0 {
                        next = next.checked_div_exact(previous_pivot)?;
                    }
                    wide_square_set(&mut cells, side, row, column, next)?;
                    column += 1;
                }
                wide_square_set(&mut cells, side, row, pivot_column, SignedWideV1::ZERO)?;
                row += 1;
            }
            previous_pivot = pivot;
            pivot_column += 1;
        }
        let determinant = wide_square_get(&cells, side, side - 1, side - 1)?;
        Ok(if sign_negative {
            determinant.negated()
        } else {
            determinant
        })
    }

    fn index(self, row: usize, column: usize) -> ResultFractionFreeV1<usize> {
        let side = usize::from(self.side);
        if row >= side || column >= side {
            return Err(FractionFreeErrorV1::InvariantViolation);
        }
        row.checked_mul(side)
            .and_then(|base| base.checked_add(column))
            .ok_or(FractionFreeErrorV1::ArithmeticOverflow)
    }
}

/// Select original row indices spanning every column of a rectangular matrix.
///
/// The returned prefix has exactly `column_count` entries and is suitable for
/// building a nonsingular square subsystem. A rank-deficient input returns
/// `None`; no approximate pivot threshold exists.
pub(crate) fn select_independent_rows_v1(
    source: &[[SignedDeltaV1; MAX_FRACTION_FREE_SIDE_V1]; MAX_FRACTION_FREE_ROWS_V1],
    row_count: usize,
    column_count: usize,
) -> ResultFractionFreeV1<Option<[u8; MAX_FRACTION_FREE_SIDE_V1]>> {
    if row_count == 0
        || row_count > MAX_FRACTION_FREE_ROWS_V1
        || column_count == 0
        || column_count > MAX_FRACTION_FREE_SIDE_V1
        || column_count > row_count
    {
        return Err(FractionFreeErrorV1::InvariantViolation);
    }
    let mut cells = [SignedWideV1::ZERO; MAX_RECTANGULAR_CELLS_V1];
    let mut original_rows = [0u8; MAX_FRACTION_FREE_ROWS_V1];
    let mut row = 0usize;
    while row < row_count {
        original_rows[row] = u8::try_from(row)
            .map_err(|_| FractionFreeErrorV1::ArithmeticOverflow)?;
        let mut column = 0usize;
        while column < column_count {
            let index = rectangular_index(row, column, column_count)?;
            cells[index] = source[row][column].widened();
            column += 1;
        }
        row += 1;
    }
    let mut previous_pivot = SignedWideV1::new(false, WideUnsignedV1::from_u64(1));
    let mut pivot_column = 0usize;
    while pivot_column < column_count {
        let mut pivot_row = pivot_column;
        while pivot_row < row_count
            && rectangular_get(&cells, pivot_row, pivot_column, column_count)?
                .magnitude
                .is_zero()
        {
            pivot_row += 1;
        }
        if pivot_row == row_count {
            return Ok(None);
        }
        if pivot_row != pivot_column {
            rectangular_swap_rows(
                &mut cells,
                &mut original_rows,
                pivot_row,
                pivot_column,
                column_count,
            )?;
        }
        let pivot = rectangular_get(&cells, pivot_column, pivot_column, column_count)?;
        row = pivot_column + 1;
        while row < row_count {
            let below = rectangular_get(&cells, row, pivot_column, column_count)?;
            let mut column = pivot_column + 1;
            while column < column_count {
                let diagonal_product =
                    pivot.checked_mul(rectangular_get(&cells, row, column, column_count)?)?;
                let cross_product = below.checked_mul(rectangular_get(
                    &cells,
                    pivot_column,
                    column,
                    column_count,
                )?)?;
                let mut next = diagonal_product.checked_sub(cross_product)?;
                if pivot_column != 0 {
                    next = next.checked_div_exact(previous_pivot)?;
                }
                rectangular_set(&mut cells, row, column, column_count, next)?;
                column += 1;
            }
            rectangular_set(
                &mut cells,
                row,
                pivot_column,
                column_count,
                SignedWideV1::ZERO,
            )?;
            row += 1;
        }
        previous_pivot = pivot;
        pivot_column += 1;
    }
    let mut selected = [0u8; MAX_FRACTION_FREE_SIDE_V1];
    selected[..column_count].copy_from_slice(&original_rows[..column_count]);
    Ok(Some(selected))
}

fn wide_square_get(
    cells: &[SignedWideV1; MAX_FRACTION_FREE_CELLS_V1],
    side: usize,
    row: usize,
    column: usize,
) -> ResultFractionFreeV1<SignedWideV1> {
    let index = row
        .checked_mul(side)
        .and_then(|base| base.checked_add(column))
        .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
    cells
        .get(index)
        .copied()
        .ok_or(FractionFreeErrorV1::InvariantViolation)
}

fn wide_square_set(
    cells: &mut [SignedWideV1; MAX_FRACTION_FREE_CELLS_V1],
    side: usize,
    row: usize,
    column: usize,
    value: SignedWideV1,
) -> ResultFractionFreeV1<()> {
    let index = row
        .checked_mul(side)
        .and_then(|base| base.checked_add(column))
        .ok_or(FractionFreeErrorV1::ArithmeticOverflow)?;
    let cell = cells
        .get_mut(index)
        .ok_or(FractionFreeErrorV1::InvariantViolation)?;
    *cell = value;
    Ok(())
}

fn wide_square_swap_rows(
    cells: &mut [SignedWideV1; MAX_FRACTION_FREE_CELLS_V1],
    side: usize,
    left: usize,
    right: usize,
) -> ResultFractionFreeV1<()> {
    if left >= side || right >= side {
        return Err(FractionFreeErrorV1::InvariantViolation);
    }
    let mut column = 0usize;
    while column < side {
        cells.swap(left * side + column, right * side + column);
        column += 1;
    }
    Ok(())
}

fn rectangular_index(
    row: usize,
    column: usize,
    column_count: usize,
) -> ResultFractionFreeV1<usize> {
    if row >= MAX_FRACTION_FREE_ROWS_V1 || column >= column_count {
        return Err(FractionFreeErrorV1::InvariantViolation);
    }
    row.checked_mul(column_count)
        .and_then(|base| base.checked_add(column))
        .ok_or(FractionFreeErrorV1::ArithmeticOverflow)
}

fn rectangular_get(
    cells: &[SignedWideV1; MAX_RECTANGULAR_CELLS_V1],
    row: usize,
    column: usize,
    column_count: usize,
) -> ResultFractionFreeV1<SignedWideV1> {
    cells
        .get(rectangular_index(row, column, column_count)?)
        .copied()
        .ok_or(FractionFreeErrorV1::InvariantViolation)
}

fn rectangular_set(
    cells: &mut [SignedWideV1; MAX_RECTANGULAR_CELLS_V1],
    row: usize,
    column: usize,
    column_count: usize,
    value: SignedWideV1,
) -> ResultFractionFreeV1<()> {
    let cell = cells
        .get_mut(rectangular_index(row, column, column_count)?)
        .ok_or(FractionFreeErrorV1::InvariantViolation)?;
    *cell = value;
    Ok(())
}

fn rectangular_swap_rows(
    cells: &mut [SignedWideV1; MAX_RECTANGULAR_CELLS_V1],
    original_rows: &mut [u8; MAX_FRACTION_FREE_ROWS_V1],
    left: usize,
    right: usize,
    column_count: usize,
) -> ResultFractionFreeV1<()> {
    if left >= MAX_FRACTION_FREE_ROWS_V1 || right >= MAX_FRACTION_FREE_ROWS_V1 {
        return Err(FractionFreeErrorV1::InvariantViolation);
    }
    let mut column = 0usize;
    while column < column_count {
        cells.swap(
            rectangular_index(left, column, column_count)?,
            rectangular_index(right, column, column_count)?,
        );
        column += 1;
    }
    original_rows.swap(left, right);
    Ok(())
}

pub(crate) fn determinant_2x2(
    left_top: SignedDeltaV1,
    right_bottom: SignedDeltaV1,
    right_top: SignedDeltaV1,
    left_bottom: SignedDeltaV1,
) -> ResultFractionFreeV1<SignedWideV1> {
    left_top
        .product(right_bottom)?
        .checked_sub(right_top.product(left_bottom)?)
}

pub(crate) fn wide_gcd(
    mut left: WideUnsignedV1,
    mut right: WideUnsignedV1,
) -> ResultFractionFreeV1<WideUnsignedV1> {
    if left.is_zero() {
        return Ok(right);
    }
    if right.is_zero() {
        return Ok(left);
    }
    let common_shift = core::cmp::min(left.trailing_zeros()?, right.trailing_zeros()?);
    left = left.shr(left.trailing_zeros()?);
    loop {
        right = right.shr(right.trailing_zeros()?);
        if left > right {
            core::mem::swap(&mut left, &mut right);
        }
        right = right
            .checked_sub(left)
            .ok_or(FractionFreeErrorV1::InvariantViolation)?;
        if right.is_zero() {
            return left
                .checked_shl(common_shift)
                .ok_or(FractionFreeErrorV1::ArithmeticOverflow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn scalar(negative: bool, magnitude: u64) -> SignedDeltaV1 {
        SignedDeltaV1 {
            negative,
            magnitude,
        }
    }

    #[test]
    fn fraction_free_determinant_pivots_and_preserves_sign() {
        let matrix = FractionFreeMatrix3V1::new([
            [scalar(false, 0), scalar(false, 2), scalar(false, 1)],
            [scalar(false, 1), scalar(false, 0), scalar(false, 3)],
            [scalar(false, 4), scalar(false, 1), scalar(false, 0)],
        ]);
        let determinant = matrix.determinant().unwrap();
        assert!(!determinant.negative);
        assert_eq!(determinant.magnitude.to_u64(), Some(25));

        let singular = FractionFreeMatrix3V1::new([
            [scalar(false, 1), scalar(false, 2), scalar(false, 3)],
            [scalar(false, 2), scalar(false, 4), scalar(false, 6)],
            [scalar(false, 0), scalar(false, 1), scalar(false, 1)],
        ]);
        assert!(singular.determinant().unwrap().magnitude.is_zero());
    }

    #[test]
    fn wide_exact_division_and_overflow_are_explicit() {
        let left = WideUnsignedV1::from_u128(u128::MAX).unwrap();
        let square = left.checked_mul(left).unwrap();
        assert!(square.limb(3).unwrap() != 0);
        let top_bit = WideUnsignedV1::from_u64(1).checked_shl(2_047).unwrap();
        assert!(top_bit.checked_mul_u64(2).is_none());
        assert_eq!(
            square.checked_div_exact(left).unwrap(),
            left,
        );
        assert_eq!(
            WideUnsignedV1::from_u64(5).checked_div_exact(WideUnsignedV1::from_u64(2)),
            Err(FractionFreeErrorV1::NonExactDivision),
        );
    }

    #[test]
    fn general_matrix_and_rectangular_rank_select_exact_rows() {
        let mut matrix = FractionFreeMatrixV1::new(5).unwrap();
        let mut diagonal = 0usize;
        while diagonal < 5 {
            matrix
                .set(diagonal, diagonal, scalar(false, u64::try_from(diagonal + 2).unwrap()))
                .unwrap();
            diagonal += 1;
        }
        assert_eq!(matrix.determinant().unwrap().magnitude.to_u64(), Some(720));

        let mut rows = [[scalar(false, 0); MAX_FRACTION_FREE_SIDE_V1];
            MAX_FRACTION_FREE_ROWS_V1];
        rows[0][0] = scalar(false, 1);
        rows[1][0] = scalar(false, 2);
        rows[1][1] = scalar(false, 1);
        rows[2][0] = scalar(false, 3);
        rows[2][1] = scalar(false, 2);
        rows[2][2] = scalar(false, 1);
        rows[3] = rows[2];
        let selected = select_independent_rows_v1(&rows, 4, 3).unwrap().unwrap();
        assert_eq!(&selected[..3], &[0, 1, 2]);

        rows[2] = rows[1];
        rows[3] = rows[1];
        assert_eq!(select_independent_rows_v1(&rows, 4, 3).unwrap(), None);
    }
}
