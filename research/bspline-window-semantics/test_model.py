"""Deterministic tests for the isolated exact window-semantics model."""

from __future__ import annotations

import importlib.util
from fractions import Fraction
from pathlib import Path
import random
import sys
import unittest


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("bspline_window_model", HERE / "model.py")
assert SPEC is not None and SPEC.loader is not None
model = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = model
SPEC.loader.exec_module(model)


class BasisAndQuantizationTests(unittest.TestCase):
    def test_exact_partition_unity_local_support_and_quantized_unity(self) -> None:
        for degree in (2, 3):
            for gap in (1, 2, 4, 8):
                for count in range(2, 7):
                    points = tuple(3 + index * gap for index in range(count))
                    for denominator in (1, 3, 7, 16, 257):
                        spec = model.BasisSpec(degree, points, denominator)
                        for quarter in range(4 * (points[0] - 1), 4 * (points[-1] + 1) + 1):
                            value = Fraction(quarter, 4)
                            exact = model.exact_basis(spec, value)
                            quantized = model.quantize_weights(spec, value)
                            self.assertEqual(sum(exact), 1)
                            self.assertTrue(all(weight >= 0 for weight in exact))
                            self.assertLessEqual(sum(weight > 0 for weight in exact), degree + 1)
                            self.assertEqual(sum(quantized), denominator)
                            self.assertTrue(all(weight >= 0 for weight in quantized))

    def test_largest_remainder_tie_breaks_toward_low_index(self) -> None:
        self.assertEqual(
            model.quantize_simplex((Fraction(1, 2), Fraction(1, 2)), 1),
            (1, 0),
        )

    def test_refusing_edge_policy_never_silently_clamps(self) -> None:
        spec = model.BasisSpec(2, (4, 8), 16, model.EdgePolicy.REFUSE)
        with self.assertRaisesRegex(model.Refusal, "value-out-of-range"):
            model.quantize_weights(spec, 3)


class IntervalSemanticsTests(unittest.TestCase):
    def test_endpoint_equality_is_not_a_degree_two_certificate(self) -> None:
        # Scaled from the exact [7/4, 9/4] counterexample on knots [0,4,8].
        spec = model.BasisSpec(2, (0, 16, 32), 4)
        self.assertEqual(model.quantize_weights(spec, 7), (1, 2, 1, 0))
        self.assertEqual(model.quantize_weights(spec, 9), (1, 2, 1, 0))
        self.assertEqual(model.quantize_weights(spec, 8), (1, 3, 0, 0))
        with self.assertRaisesRegex(model.Refusal, "ambiguous-interval"):
            model.certify_constant_integer_interval(spec, 7, 9)

    def test_exact_bounded_scan_accepts_only_a_constant_integer_lattice(self) -> None:
        spec = model.BasisSpec(3, (0, 64, 128), 2)
        certificate = model.certify_constant_integer_interval(spec, 1, 2)
        self.assertEqual(certificate.evaluated_points, 2)
        self.assertEqual(certificate.lattice, "integer")
        self.assertEqual(certificate.vector, model.quantize_weights(spec, 1))

    def test_scan_budget_refuses_instead_of_sampling(self) -> None:
        spec = model.BasisSpec(2, (0, 64), 8)
        with self.assertRaisesRegex(model.Refusal, "interval-certificate-budget"):
            model.certify_constant_integer_interval(spec, 0, 64, max_points=8)

    def test_rational_twap_interval_scans_every_compatible_numerator(self) -> None:
        spec = model.BasisSpec(3, (0, 64), 2)
        certificate = model.certify_constant_ratio_interval(spec, 1, 2, 4)
        self.assertEqual(certificate.evaluated_points, 2)
        self.assertEqual(certificate.low, Fraction(1, 4))
        self.assertEqual(certificate.high, Fraction(1, 2))

    def test_confidence_policy_is_semantic_and_frozen(self) -> None:
        spec = model.BasisSpec(2, (0, 16, 32), 4)
        point = model.resolve_source_point(
            spec, 8, 7, 9, model.ConfidencePolicy.POINT_AUTHORITATIVE
        )
        self.assertEqual(point, (1, 3, 0, 0))
        with self.assertRaisesRegex(model.Refusal, "ambiguous-interval"):
            model.resolve_source_point(
                spec, 8, 7, 9, model.ConfidencePolicy.PAYOUT_INVARIANT
            )


class PathSemanticsTests(unittest.TestCase):
    def test_constant_path_agrees_in_all_modes(self) -> None:
        spec = model.BasisSpec(3, (0, 8, 16, 24), 257)
        comparison = model.compare_path_modes(
            spec, ((10, 3, 9), (11, 5, 9), (12, 7, 9))
        )
        self.assertEqual(comparison.evaluate_at_twap, comparison.quantized_basis_occupation)
        self.assertEqual(comparison.evaluate_at_twap, comparison.exact_basis_occupation)

    def test_path_modes_are_not_synonyms(self) -> None:
        spec = model.BasisSpec(2, (0, 16, 32), 64)
        comparison = model.compare_path_modes(spec, ((0, 1, 4), (1, 1, 28)))
        self.assertEqual(comparison.price_time_integral, 32)
        self.assertEqual(comparison.duration, 2)
        self.assertNotEqual(
            comparison.evaluate_at_twap, comparison.exact_basis_occupation
        )
        for vector in (
            comparison.evaluate_at_twap,
            comparison.quantized_basis_occupation,
            comparison.exact_basis_occupation,
        ):
            self.assertEqual(sum(vector), spec.denominator)

    def test_local_quantization_can_change_occupation_atoms(self) -> None:
        spec = model.BasisSpec(2, (0, 8, 16), 7)
        comparison = model.compare_path_modes(spec, ((0, 1, 0), (1, 1, 4)))
        self.assertEqual(comparison.quantized_basis_occupation, (5, 2, 0, 0))
        self.assertEqual(comparison.exact_basis_occupation, (4, 2, 1, 0))
        self.assertEqual(comparison.evaluate_at_twap, (4, 3, 0, 0))

    def test_twap_and_occupation_monoids_are_associative(self) -> None:
        spec = model.BasisSpec(3, (0, 8, 16, 24), 31)
        observations = ((4, 2, 3), (5, 7, 11), (6, 5, 21))
        constructors = (
            model.TwapSummary.from_point,
            model.QuantizedOccupationSummary.from_point,
            model.ExactBasisOccupationSummary.from_point,
        )
        for constructor in constructors:
            a, b, c = (
                constructor(spec, bucket, duration, point)
                for bucket, duration, point in observations
            )
            self.assertEqual(a.combine(b).combine(c), a.combine(b.combine(c)))

    def test_empty_is_identity_and_nonadjacent_refuses(self) -> None:
        spec = model.BasisSpec(2, (0, 8), 8)
        part = model.QuantizedOccupationSummary.from_point(spec, 5, 1, 4)
        empty = model.QuantizedOccupationSummary.empty(spec)
        self.assertEqual(empty.combine(part), part)
        self.assertEqual(part.combine(empty), part)
        later = model.QuantizedOccupationSummary.from_point(spec, 7, 1, 4)
        with self.assertRaisesRegex(model.Refusal, "nonadjacent"):
            part.combine(later)

    def test_exact_basis_scale_clears_every_integer_point(self) -> None:
        for degree in (2, 3):
            for gap in (1, 2, 4, 8):
                spec = model.BasisSpec(degree, tuple(index * gap for index in range(5)), 19)
                scale = model.exact_integer_basis_scale(spec)
                for point in range(spec.breakpoints[-1] + 1):
                    for weight in model.exact_basis(spec, point):
                        self.assertEqual((weight * scale).denominator, 1)

    def test_interval_occupation_uses_certificate_not_midpoint(self) -> None:
        spec = model.BasisSpec(2, (0, 16, 32), 4)
        with self.assertRaisesRegex(model.Refusal, "ambiguous-interval"):
            model.QuantizedOccupationSummary.from_interval(spec, 0, 1, 7, 9)


class SolvencyTests(unittest.TestCase):
    def test_all_modes_inherit_simplex_solvency(self) -> None:
        random_source = random.Random(0xD6E66)
        for degree in (2, 3):
            spec = model.BasisSpec(degree, (0, 8, 16, 24), 257)
            comparison = model.compare_path_modes(
                spec, ((0, 2, 3), (1, 5, 12), (2, 7, 22))
            )
            for vector in (
                comparison.evaluate_at_twap,
                comparison.quantized_basis_occupation,
                comparison.exact_basis_occupation,
            ):
                for _ in range(100):
                    supplies = tuple(
                        random_source.randrange(1_000_000)
                        for _ in range(spec.outcome_count)
                    )
                    model.assert_simplex_solvency(vector, spec.denominator, supplies)


if __name__ == "__main__":
    unittest.main()
