# SPDX-License-Identifier: AGPL-3.0-or-later
"""Kernel-alignment and payout-candidate falsifiers.

Covers POLICY_ANALYSIS_LOTS_FEES.md section 3.1/3.2 (same admitted market set,
same payout semantics) and the EXP-LOT rows of its section 5 matrix.  Each
experiment test asserts the row's own falsification condition and that its
enumeration was not vacuous.
"""

from __future__ import annotations

import unittest
from fractions import Fraction

from experiments import (
    exp_lot_a1,
    exp_lot_b1,
    exp_lot_b2,
    exp_lot_b3,
    exp_lot_b4,
    exp_lot_b5,
    exp_lot_c1,
    exp_lot_c2,
    exp_lot_x1,
    exp_lot_x2,
)
from model import (
    IntegerPayoutSet,
    IntegerPayoutVector,
    KernelRefusal,
    ModelError,
    PayoutPolicy,
    WeightedBook,
    compatible_payout,
    enumerate_weighted_traces,
    maximum_liability,
    one_hot_payout_set,
    one_hot_vectors,
    payout_set,
)


class AdmittedMarketSet(unittest.TestCase):
    """Section 3.1: the lab moves to the kernel's weight-sum equality."""

    def test_weight_sums_below_one_are_refused(self) -> None:
        with self.assertRaises(ModelError):
            maximum_liability((10, 10), [(Fraction(1, 2), Fraction(1, 4))])

    def test_weight_sums_above_one_are_refused(self) -> None:
        with self.assertRaises(ModelError):
            maximum_liability((10, 10), [(Fraction(3, 4), Fraction(1, 2))])

    def test_exactly_summing_families_are_still_admitted(self) -> None:
        supplies = (10, 7, 3)
        payouts = (*one_hot_vectors(3), compatible_payout(3, (0, 2)))
        self.assertEqual(maximum_liability(supplies, payouts), 10)

    def test_integer_mirror_refuses_the_kernel_shape_violations(self) -> None:
        cases = {
            "invalid_outcome_count": IntegerPayoutSet(
                1, 1, (IntegerPayoutVector(1, (1,)),)
            ),
            "invalid_payout_count": IntegerPayoutSet(
                0, 2, (IntegerPayoutVector(1, (1, 0)),)
            ),
            "invalid_denominator": IntegerPayoutSet(
                2,
                2,
                (IntegerPayoutVector(1, (1, 0)), IntegerPayoutVector(2, (1, 1))),
            ),
            "invalid_payout_weights": IntegerPayoutSet(
                1, 2, (IntegerPayoutVector(1, (1, 0, 1)),)
            ),
        }
        for expected, candidate in cases.items():
            with self.subTest(expected=expected):
                with self.assertRaises(KernelRefusal) as caught:
                    candidate.validate()
                self.assertEqual(caught.exception.error_class, expected)

    def test_negative_weights_are_refused_as_weight_defects(self) -> None:
        # The kernel weight type is u64, so this input is unrepresentable there;
        # the lab classifies it rather than crashing.
        with self.assertRaises(KernelRefusal) as caught:
            IntegerPayoutSet(1, 2, (IntegerPayoutVector(1, (2, -1)),)).validate()
        self.assertEqual(caught.exception.error_class, "invalid_payout_weights")

    def test_weighted_mirror_is_integer_only(self) -> None:
        # PROPOSED variant, explicitly named (P0-5)
        book = (
            WeightedBook.open(payout_set(2, [[1, 1]], 2), PayoutPolicy.KERNEL_BASELINE)
            .split(0, 4)
            .resolve(0)
        )
        book, payout = book.redeem_internal(0, 0, 2)
        self.assertIsInstance(payout, int)
        self.assertIsInstance(book.collateral, int)
        self.assertIsInstance(book.required_collateral(), int)
        self.assertNotIsInstance(payout, Fraction)


class LotArithmetic(unittest.TestCase):
    def test_one_hot_sets_have_unit_lots(self) -> None:
        for outcomes in range(2, 6):
            for denominator in (1, 2, 7):
                candidate = one_hot_payout_set(outcomes, denominator)
                self.assertEqual(candidate.split_lot(), 1)
                for index in range(outcomes):
                    self.assertEqual(candidate.redemption_lot(index), 1)

    def test_p1a_set_has_a_two_atom_lot(self) -> None:
        candidate = payout_set(2, [[1, 1]], 2)
        self.assertEqual(candidate.redemption_lot(0), 2)
        self.assertEqual(candidate.redemption_lot(1), 2)
        self.assertEqual(candidate.split_lot(), 2)

    def test_exp_lot_b1_lot_formula_is_the_minimal_modulus(self) -> None:
        result = exp_lot_b1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["sets"], 500)
        self.assertGreater(result.counts["quantities"], 10_000)

    def test_exp_lot_b4_lot_magnitude_table(self) -> None:
        result = exp_lot_b4()
        self.assertFalse(result.falsified, msg=result.witnesses)
        rows = {row["family"]: row for row in result.data["rows"]}
        self.assertEqual(rows["sizes_2_to_8"]["denominator"], 840)
        self.assertEqual(rows["sizes_2_to_8"]["split_lot"], 840)
        self.assertEqual(rows["one_hot_only"]["split_lot"], 1)


class PayoutCandidateArms(unittest.TestCase):
    """Section 3.2: one arm per candidate, checked for exit-liveness, not just solvency."""

    def test_p1a_trap_behaves_differently_in_every_arm(self) -> None:
        trap = payout_set(2, [[1, 1]], 2)
        with self.assertRaises(KernelRefusal) as caught:
            WeightedBook.open(trap, PayoutPolicy.ONE_HOT)
        self.assertEqual(caught.exception.error_class, "invalid_payout_weights")

        with self.assertRaises(KernelRefusal) as caught:
            WeightedBook.open(trap, PayoutPolicy.LOTS).split(0, 1)
        self.assertEqual(caught.exception.error_class, "lot_violation")

        # PROPOSED variant, explicitly named (P0-5)
        baseline = (
            WeightedBook.open(trap, PayoutPolicy.KERNEL_BASELINE).split(0, 1).resolve(0)
        )
        with self.assertRaises(KernelRefusal) as caught:
            baseline.redeem_internal(0, 0, 1)
        self.assertEqual(caught.exception.error_class, "remainder_required")
        self.assertTrue(baseline.is_solvent())
        self.assertEqual(baseline.retirement_residue(terminal_complete_set=False), 2)

        credit = WeightedBook.open(trap, PayoutPolicy.CREDIT).split(0, 1).resolve(0)
        credit, paid = credit.redeem_internal(0, 0, 1)
        self.assertEqual(paid, 0)
        self.assertEqual(credit.positions[0].credit, 1)

    def test_bounded_walk_reports_exit_liveness_per_arm(self) -> None:
        trap = payout_set(2, [[1, 1]], 2)
        baseline = enumerate_weighted_traces(trap, PayoutPolicy.KERNEL_BASELINE, depth=4)
        self.assertGreater(baseline["exit_dead_states"], 0)
        self.assertGreater(baseline["refusals"]["remainder_required"], 0)
        credit = enumerate_weighted_traces(trap, PayoutPolicy.CREDIT, depth=4)
        self.assertEqual(credit["exit_dead_states"], 0)
        self.assertNotIn("remainder_required", credit["refusals"])
        lots = enumerate_weighted_traces(trap, PayoutPolicy.LOTS, depth=4)
        self.assertEqual(lots["exit_dead_states"], 0)
        self.assertGreater(lots["refusals"]["lot_violation"], 0)

    def test_exp_lot_a1_one_hot_admission(self) -> None:
        result = exp_lot_a1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["admitted"], 50)
        self.assertGreater(result.counts["refused"], 100)
        self.assertGreater(result.counts["walked_states"], 100)

    def test_exp_lot_b2_internal_closure(self) -> None:
        result = exp_lot_b2()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertEqual(result.data["p1a_split_refusal"], "lot_violation")
        self.assertGreater(result.counts["states"], 1_000)

    def test_exp_lot_b3_external_fragmentation_adversary(self) -> None:
        result = exp_lot_b3()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["schedules"], 10)
        self.assertGreater(result.counts["max_trapped_numerator"], 0)

    def test_exp_lot_b5_lot_aligned_reservation_is_exact(self) -> None:
        result = exp_lot_b5()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["states"], 1_000)

    def test_exp_lot_c1_credit_conservation(self) -> None:
        result = exp_lot_c1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["redemptions"], 1_000)
        self.assertGreater(result.counts["fragmentations"], 1_000)

    def test_exp_lot_c2_fragmented_redemption_has_no_arbitrage(self) -> None:
        result = exp_lot_c2()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["schedules"], 1_000)

    def test_exp_lot_x1_terminal_complete_set_redemption(self) -> None:
        result = exp_lot_x1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["redemptions"], 1_000)
        self.assertEqual(result.data["p1a_payout_with_primitive"], 1)
        self.assertEqual(result.data["p1a_residue_without_primitive"], 2)

    def test_exp_lot_x2_retirement_liveness_per_arm(self) -> None:
        result = exp_lot_x2()
        self.assertFalse(result.falsified, msg=result.witnesses)
        rows = {row["arm"]: row for row in result.data["rows"]}
        self.assertEqual(rows["one_hot"]["unretireable_with_complete_set_primitive"], 0)
        self.assertGreater(
            rows["lots_b1_external"]["unretireable_with_complete_set_primitive"], 0
        )
        self.assertGreater(
            rows["kernel_baseline_fractional"]["unretireable_without_primitive"], 0
        )


if __name__ == "__main__":
    unittest.main()
