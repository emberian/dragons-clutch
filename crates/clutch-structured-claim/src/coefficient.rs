//! Exact rational-to-integral realization and complete-set decomposition.

use core::convert::TryFrom;

use crate::{gcd_u128, gcd_u64, Amount, Error, Result, MAX_OUTCOMES, MIN_OUTCOMES};

/// One canonical nonnegative exact rational coefficient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct RationalCoefficient {
    /// Nonnegative numerator.
    pub numerator: u64,
    /// Positive denominator.
    pub denominator: u64,
}

impl RationalCoefficient {
    /// Canonical exact zero.
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    /// Construct a rational; admission still occurs in [`RationalShape::validate`].
    pub const fn new(numerator: u64, denominator: u64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    fn validate(&self) -> Result<()> {
        if self.denominator == 0 {
            return Err(Error::InvalidDenominator);
        }
        if self.numerator == 0 {
            if self.denominator != 1 {
                return Err(Error::NonCanonicalRational);
            }
        } else if gcd_u64(self.numerator, self.denominator) != 1 {
            return Err(Error::NonCanonicalRational);
        }
        Ok(())
    }
}

/// Fixed-capacity exact coefficient input from an analytic compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct RationalShape {
    /// Active coefficient prefix width.
    pub outcome_count: u8,
    /// Reduced active rationals followed by canonical `0/1` padding.
    pub coefficients: [RationalCoefficient; MAX_OUTCOMES],
}

impl RationalShape {
    /// Validate reduced rationals, width, and canonical padding.
    pub fn validate(&self) -> Result<()> {
        let count = usize::from(self.outcome_count);
        if self.outcome_count < MIN_OUTCOMES || count > MAX_OUTCOMES {
            return Err(Error::InvalidOutcomeCount);
        }
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let coefficient = self.coefficients[index];
            if index < count {
                coefficient.validate()?;
            } else if coefficient != RationalCoefficient::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Canonical primitive nonnegative vector for one transferable product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ClaimVector {
    /// Active native Egg width.
    pub outcome_count: u8,
    /// GCD-one active prefix followed by zero padding.
    pub coefficients: [Amount; MAX_OUTCOMES],
}

impl ClaimVector {
    /// Validate primitive normalization, padding, and nontrivial wrapper value.
    pub fn validate(&self) -> Result<()> {
        let count = usize::from(self.outcome_count);
        if self.outcome_count < MIN_OUTCOMES || count > MAX_OUTCOMES {
            return Err(Error::InvalidOutcomeCount);
        }
        let mut divisor = 0_u64;
        let mut support = 0_u8;
        let first = self.coefficients[0];
        let mut constant = true;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let value = self.coefficients[index];
            if index < count {
                divisor = gcd_u64(divisor, value);
                if value != 0 {
                    support = support.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                }
                if value != first {
                    constant = false;
                }
            } else if value != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if divisor == 0 {
            return Err(Error::ZeroClaim);
        }
        if divisor != 1 {
            return Err(Error::NonPrimitiveClaim);
        }
        if support < 2 {
            return Err(Error::SingleEggClaim);
        }
        if constant {
            return Err(Error::CompleteSetClaim);
        }
        Ok(())
    }

    /// Derive the unique complete-set-compressed backing for one wrapper.
    pub fn backing_plan(&self) -> Result<BackingPlan> {
        self.validate()?;
        let count = usize::from(self.outcome_count);
        let mut floor = self.coefficients[0];
        let mut index = 1_usize;
        while index < count {
            if self.coefficients[index] < floor {
                floor = self.coefficients[index];
            }
            index += 1;
        }
        let mut residual = [0_u64; MAX_OUTCOMES];
        index = 0;
        while index < count {
            residual[index] = self.coefficients[index]
                .checked_sub(floor)
                .ok_or(Error::ArithmeticUnderflow)?;
            index += 1;
        }
        Ok(BackingPlan {
            outcome_count: self.outcome_count,
            cash_per_wrapper: floor,
            residual_eggs_per_wrapper: residual,
        })
    }
}

/// Canonical native backing owed for each wrapper atom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct BackingPlan {
    /// Active native Egg width.
    pub outcome_count: u8,
    /// Complete-set floor held as free base Position cash.
    pub cash_per_wrapper: Amount,
    /// Residual native Eggs, with at least one active zero entry.
    pub residual_eggs_per_wrapper: [Amount; MAX_OUTCOMES],
}

/// Minimal exact integral realization of an analytic rational vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct IntegerRealization {
    /// Primitive native wrapper claim.
    pub claim: ClaimVector,
    /// Wrapper atoms representing one exact display lot.
    pub wrapper_atoms_per_display_lot: Amount,
    /// Units of the rational target represented by that display lot.
    pub target_units_per_display_lot: Amount,
    /// Canonical complete-set-compressed backing.
    pub backing: BackingPlan,
}

/// Compile reduced exact rationals without rounding or heap allocation.
///
/// The common denominator and integerized numerators use checked `u128`.
/// Output primitive coefficients and the minimal display ratio must fit `u64`;
/// otherwise the production amount domain refuses the input.
pub fn realize_rational_shape(shape: &RationalShape) -> Result<IntegerRealization> {
    shape.validate()?;
    let count = usize::from(shape.outcome_count);
    let mut common_denominator = 1_u128;
    let mut index = 0_usize;
    while index < count {
        let denominator = u128::from(shape.coefficients[index].denominator);
        let divisor = gcd_u128(common_denominator, denominator);
        common_denominator = common_denominator
            .checked_div(divisor)
            .and_then(|value| value.checked_mul(denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }

    let mut integerized = [0_u128; MAX_OUTCOMES];
    let mut coefficient_gcd = 0_u128;
    index = 0;
    while index < count {
        let coefficient = shape.coefficients[index];
        let scale = common_denominator
            .checked_div(u128::from(coefficient.denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        let value = u128::from(coefficient.numerator)
            .checked_mul(scale)
            .ok_or(Error::ArithmeticOverflow)?;
        integerized[index] = value;
        coefficient_gcd = gcd_u128(coefficient_gcd, value);
        index += 1;
    }
    if coefficient_gcd == 0 {
        return Err(Error::ZeroClaim);
    }

    let ratio_gcd = gcd_u128(common_denominator, coefficient_gcd);
    let wrapper_atoms_per_display_lot = Amount::try_from(
        coefficient_gcd
            .checked_div(ratio_gcd)
            .ok_or(Error::ArithmeticOverflow)?,
    )
    .map_err(|_| Error::ArithmeticOverflow)?;
    let target_units_per_display_lot = Amount::try_from(
        common_denominator
            .checked_div(ratio_gcd)
            .ok_or(Error::ArithmeticOverflow)?,
    )
    .map_err(|_| Error::ArithmeticOverflow)?;

    let mut primitive = [0_u64; MAX_OUTCOMES];
    index = 0;
    while index < count {
        primitive[index] = Amount::try_from(
            integerized[index]
                .checked_div(coefficient_gcd)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .map_err(|_| Error::ArithmeticOverflow)?;
        index += 1;
    }
    let claim = ClaimVector {
        outcome_count: shape.outcome_count,
        coefficients: primitive,
    };
    claim.validate()?;
    let backing = claim.backing_plan()?;
    Ok(IntegerRealization {
        claim,
        wrapper_atoms_per_display_lot,
        target_units_per_display_lot,
        backing,
    })
}
