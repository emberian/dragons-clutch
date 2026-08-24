//! Exact integer arithmetic shared by protocol allocation policies.
//!
//! These helpers avoid relying on a double-width intermediate, so callers can
//! apply one checked Hamilton boundary to hostile `u128` weights without
//! truncating the weights first or overflowing `multiplicand * multiplier`.

/// Refusal from an exact-integer operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactIntegerError {
    /// Division by zero has no protocol meaning.
    ZeroDenominator,
    /// The exact mathematical quotient cannot be represented by `u128`.
    QuotientOverflow,
}

/// Compute the quotient and remainder of
/// `multiplicand * multiplier / denominator` without forming the product.
///
/// Modular doubling keeps every intermediate remainder below `denominator`,
/// including for hostile values near `u128::MAX`. The function is total over
/// its input domain: it refuses a zero denominator and a mathematical quotient
/// wider than `u128`, and otherwise returns the unique `(quotient, remainder)`
/// satisfying `product = quotient * denominator + remainder` with
/// `remainder < denominator`.
pub fn exact_mul_div_rem(
    multiplicand: u128,
    multiplier: u128,
    denominator: u128,
) -> Result<(u128, u128), ExactIntegerError> {
    if denominator == 0 {
        return Err(ExactIntegerError::ZeroDenominator);
    }
    let mut quotient = 0u128;
    let mut remainder = 0u128;
    let multiplicand_quotient = multiplicand / denominator;
    let multiplicand_remainder = multiplicand % denominator;
    let mut bit = 1u128 << 127;
    loop {
        quotient = quotient
            .checked_mul(2)
            .ok_or(ExactIntegerError::QuotientOverflow)?;
        let double_complement = denominator - remainder;
        let (mut carry, mut next_remainder) = if remainder >= double_complement {
            (1u128, remainder - double_complement)
        } else {
            (0u128, remainder + remainder)
        };
        if multiplier & bit != 0 {
            carry = carry
                .checked_add(multiplicand_quotient)
                .ok_or(ExactIntegerError::QuotientOverflow)?;
            let add_complement = denominator - next_remainder;
            if multiplicand_remainder >= add_complement {
                carry = carry
                    .checked_add(1)
                    .ok_or(ExactIntegerError::QuotientOverflow)?;
                next_remainder = multiplicand_remainder - add_complement;
            } else {
                next_remainder += multiplicand_remainder;
            }
        }
        quotient = quotient
            .checked_add(carry)
            .ok_or(ExactIntegerError::QuotientOverflow)?;
        remainder = next_remainder;
        if bit == 1 {
            break;
        }
        bit >>= 1;
    }
    Ok((quotient, remainder))
}
