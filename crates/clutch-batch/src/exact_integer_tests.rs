//! Adversarial tests for shared exact integer arithmetic.

extern crate std;

use crate::exact_integer::{exact_mul_div_rem, ExactIntegerError};

#[test]
fn exact_mul_div_matches_small_products_and_double_width_cases() {
    let mut multiplicand = 0u128;
    while multiplicand < 33 {
        let mut multiplier = 0u128;
        while multiplier < 33 {
            let mut denominator = 1u128;
            while denominator < 33 {
                let product = multiplicand * multiplier;
                assert_eq!(
                    exact_mul_div_rem(multiplicand, multiplier, denominator).unwrap(),
                    (product / denominator, product % denominator)
                );
                denominator += 1;
            }
            multiplier += 1;
        }
        multiplicand += 1;
    }

    let weight = u128::from(u64::MAX);
    let denominator = weight * 32;
    let multiplicand = denominator - 1;
    assert!(multiplicand.checked_mul(weight).is_none());
    assert_eq!(
        exact_mul_div_rem(multiplicand, weight, denominator).unwrap(),
        (weight - 1, denominator - weight)
    );
    assert_eq!(
        exact_mul_div_rem(u128::MAX, u128::MAX - 1, u128::MAX).unwrap(),
        (u128::MAX - 1, 0)
    );
}

#[test]
fn exact_mul_div_refuses_zero_denominator_and_wide_quotient() {
    assert_eq!(
        exact_mul_div_rem(1, 1, 0),
        Err(ExactIntegerError::ZeroDenominator)
    );
    assert_eq!(
        exact_mul_div_rem(u128::MAX, u128::MAX, 1),
        Err(ExactIntegerError::QuotientOverflow)
    );
}
