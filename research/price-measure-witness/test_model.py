from fractions import Fraction
import unittest

from model import (
    atomic_witness,
    basis_values,
    bernstein_values,
    spline_local_power_coefficients,
    transfer_table,
    v1b_degree_two_accepts,
    validate_witness,
)


class PriceMeasureWitnessTests(unittest.TestCase):
    def assert_valid(self, degree, breakpoints, atoms):
        scale, prices, denominator, moments = atomic_witness(degree, breakpoints, atoms)
        validate_witness(
            degree=degree,
            breakpoint_count=breakpoints,
            price_scale=scale,
            prices=prices,
            common_denominator=denominator,
            moments=moments,
        )
        return scale, prices, denominator, moments

    def test_transfer_tables_reproduce_every_interpolation_node(self):
        for degree in (2, 3):
            for breakpoints in range(2, 8):
                table = transfer_table(degree, breakpoints)
                for span, rows in enumerate(table):
                    for numerator in range(degree + 1):
                        u = Fraction(numerator, degree)
                        actual = basis_values(degree, breakpoints, span + u)
                        bernstein = bernstein_values(degree, u)
                        for outcome, row in enumerate(rows):
                            self.assertEqual(
                                sum(coefficient * weight for coefficient, weight in zip(row, bernstein)),
                                actual[outcome],
                            )

    def test_point_atoms_and_cross_span_mixtures_validate(self):
        for degree in (2, 3):
            for breakpoints in (2, 3, 5, 7):
                high = Fraction(breakpoints - 1)
                for x in (Fraction(0), Fraction(1, 3), high / 2, high):
                    self.assert_valid(degree, breakpoints, ((x, Fraction(1)),))
                self.assert_valid(
                    degree,
                    breakpoints,
                    (
                        (Fraction(0), Fraction(1, 5)),
                        (high / 3, Fraction(2, 5)),
                        (high, Fraction(2, 5)),
                    ),
                )

    def test_price_and_moment_mutations_refuse(self):
        scale, prices, denominator, moments = self.assert_valid(
            2,
            5,
            ((Fraction(1, 3), Fraction(1, 2)), (Fraction(7, 3), Fraction(1, 2))),
        )
        bad_prices = list(prices)
        source = next(index for index, value in enumerate(bad_prices) if value)
        target = (source + 1) % len(bad_prices)
        bad_prices[source] -= 1
        bad_prices[target] += 1
        with self.assertRaisesRegex(ValueError, "price reconstruction"):
            validate_witness(
                degree=2,
                breakpoint_count=5,
                price_scale=scale,
                prices=bad_prices,
                common_denominator=denominator,
                moments=moments,
            )

        bad_moments = [list(row) for row in moments]
        source_span, source_index = next(
            (span, index)
            for span, row in enumerate(bad_moments)
            for index, value in enumerate(row)
            if value
        )
        bad_moments[source_span][source_index] -= 1
        bad_moments[-1][-1] += 1
        with self.assertRaises(ValueError):
            validate_witness(
                degree=2,
                breakpoint_count=5,
                price_scale=scale,
                prices=prices,
                common_denominator=denominator,
                moments=bad_moments,
            )

    def test_noncanonical_common_scale_refuses(self):
        scale, prices, denominator, moments = self.assert_valid(
            3, 4, ((Fraction(1, 2), Fraction(1)),)
        )
        doubled = tuple(tuple(2 * value for value in row) for row in moments)
        with self.assertRaisesRegex(ValueError, "noncanonical denominator"):
            validate_witness(
                degree=3,
                breakpoint_count=4,
                price_scale=scale,
                prices=prices,
                common_denominator=2 * denominator,
                moments=doubled,
            )

    def test_named_v1b_false_acceptance_has_a_nonnegative_negative_price_portfolio(self):
        scale = 12
        prices = (4, 8, 0, 0, 0)
        coefficients = (1, -2, 10, 40, 64)
        self.assertTrue(v1b_degree_two_accepts(prices, scale))
        self.assertEqual(sum(c * p for c, p in zip(coefficients, prices)), -scale)

        # On local coordinate u in span k, the same portfolio is exactly
        # (3*(k+u)-1)^2. Equality of all three quadratic power coefficients on
        # every span proves nonnegativity over the whole continuous domain.
        for span in range(3):
            actual = spline_local_power_coefficients(2, 4, coefficients, span)
            offset = 3 * span - 1
            expected = (Fraction(offset * offset), Fraction(6 * offset), Fraction(9))
            self.assertEqual(actual, expected)


if __name__ == "__main__":
    unittest.main()
