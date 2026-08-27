# SPDX-License-Identifier: AGPL-3.0-or-later
"""Differential-fixture tests: EXP-ALIGN-01, EXP-ALIGN-02, EXP-ALIGN-03.

These replay the language-neutral vectors in ``fixtures/economics/`` through the
Python lab.  The same files are the contract for the Rust side; a divergence on
either side is a finding, never a reason to edit a fixture.
"""

from __future__ import annotations

import json
import unittest
from fractions import Fraction

from fixtures import (
    ADMISSION_VECTORS,
    TRANSITION_CLASSES,
    FEE_VECTORS,
    FIXTURE_FILES,
    TRACE_VECTORS,
    build_admission_fixture,
    build_fee_fixture,
    build_trace_fixture,
    canonical_bytes,
    classify_admission,
    fixture_directory,
    payout_set_from_fixture,
    replay_fee_vector,
    replay_trace,
)
from model import (
    ERROR_CLASSES,
    PAYOUT_POLICIES,
    KernelRefusal,
    PayoutPolicy,
    WeightedBook,
)


class AdmissionVectors(unittest.TestCase):
    """EXP-ALIGN-01: the lab and the kernel must classify identical inputs alike."""

    def test_every_admission_vector_classifies_as_specified(self) -> None:
        self.assertTrue(ADMISSION_VECTORS)
        for vector in ADMISSION_VECTORS:
            for arm in PAYOUT_POLICIES:
                with self.subTest(vector=vector["id"], arm=arm):
                    self.assertEqual(
                        classify_admission(vector, arm), vector["expected"][arm]
                    )

    def test_sub_unit_and_super_unit_weight_sums_are_both_refused(self) -> None:
        below = next(item for item in ADMISSION_VECTORS if item["id"] == "ADM-003")
        above = next(item for item in ADMISSION_VECTORS if item["id"] == "ADM-004")
        for vector in (below, above):
            for arm in PAYOUT_POLICIES:
                self.assertEqual(
                    classify_admission(vector, arm)["error_class"],
                    "invalid_payout_weights",
                )

    def test_derived_lots_match_the_fixture(self) -> None:
        checked = 0
        for vector in ADMISSION_VECTORS:
            if "derived_lots" not in vector:
                continue
            checked += 1
            payouts = payout_set_from_fixture(vector)
            self.assertEqual(
                [
                    payouts.redemption_lot(index)
                    for index in range(int(vector["outcomes"]))
                ],
                vector["derived_lots"]["redemption_lots"],
            )
            self.assertEqual(payouts.split_lot(), vector["derived_lots"]["split_lot"])
        self.assertGreaterEqual(checked, 5)


class TraceVectors(unittest.TestCase):
    """EXP-ALIGN-02: per-step results and final state under every policy arm."""

    def test_p1a_is_trace_vector_one(self) -> None:
        first = TRACE_VECTORS[0]
        self.assertEqual(first["id"], "TRC-001")
        self.assertEqual(
            first["market"]["payout_vectors"], [{"denominator": 2, "weights": [1, 1]}]
        )
        self.assertEqual(first["steps"][0], {"op": "split", "wallet": 0, "quantity": 1})

    def test_every_trace_vector_replays_as_specified(self) -> None:
        for vector in TRACE_VECTORS:
            for arm, expected in vector["arms"].items():
                with self.subTest(vector=vector["id"], arm=arm):
                    actual = replay_trace(vector, arm)
                    self.assertEqual(actual["admitted"], expected["admitted"])
                    if not expected["admitted"]:
                        self.assertEqual(
                            actual["error_class"], expected["error_class"]
                        )
                        continue
                    self.assertEqual(actual["results"], expected["results"])
                    self.assertEqual(actual["final"], expected["final"])
                    if "exit_dead" in expected:
                        self.assertEqual(actual["exit_dead"], expected["exit_dead"])

    def test_every_arm_is_covered_by_every_trace_vector(self) -> None:
        for vector in TRACE_VECTORS:
            self.assertEqual(sorted(vector["arms"]), sorted(PAYOUT_POLICIES))

    def test_p1a_is_solvent_but_stuck_under_the_landed_transitions(self) -> None:
        """The trap is exit-dead exactly when the section 1.5 primitive is absent."""

        vector = TRACE_VECTORS[0]
        book = WeightedBook.open(
            payout_set_from_fixture(vector["market"]), PayoutPolicy.KERNEL_BASELINE
        )
        for step in vector["steps"][:2]:
            book, _ = book.apply(step)
        self.assertEqual(book.collateral, 1)
        self.assertTrue(book.is_solvent())
        for step in vector["steps"][2:4]:
            with self.assertRaises(KernelRefusal) as caught:
                book.apply(step)
            self.assertEqual(caught.exception.error_class, "remainder_required")
        # Without the proposed complete-set exit no claim atom can ever be burned.
        self.assertEqual(book.retirement_residue(terminal_complete_set=False), 2)
        self.assertEqual(book.retirement_residue(terminal_complete_set=True), 0)

    def test_every_step_declares_a_transition_class(self) -> None:
        payload = build_trace_fixture()
        for vector in payload["vectors"]:
            for step in vector["steps"]:
                self.assertIn(step["transition_class"], set(TRANSITION_CLASSES))


class FeeVectors(unittest.TestCase):
    """EXP-ALIGN-03: paid/carry, allocation, and payer-debit deltas."""

    def test_every_fee_vector_replays_as_specified(self) -> None:
        for vector in FEE_VECTORS:
            with self.subTest(vector=vector["id"]):
                actual = replay_fee_vector(vector)
                for key, value in vector["expected"].items():
                    self.assertEqual(actual[key], value, msg=f"{vector['id']}.{key}")

    def test_fee_schedules_conserve_and_match_exact_rational_arithmetic(self) -> None:
        """An independent Fraction cross-check of the integer carry engine."""

        checked = 0
        for vector in FEE_VECTORS:
            if vector["kind"] != "single_egg_schedule":
                continue
            checked += 1
            scale = int(vector["price_scale"])
            kappa = Fraction(int(vector["kappa_num"]), int(vector["kappa_den"]))
            exact = Fraction(0)
            for fill in vector["fills"]:
                quantity = Fraction(int(fill["quantity"]))
                price = Fraction(int(fill["price"]), scale)
                exact += kappa * quantity * price * (1 - price)
            if vector["fee_side_arm"] == "per_intent_both_sides":
                exact *= 2
            expected_numerator = int(vector["expected"]["fee_numerator_total"])
            denominator = int(vector["expected"]["denominator"])
            self.assertEqual(Fraction(expected_numerator, denominator), exact)
            actual = replay_fee_vector(vector)
            self.assertTrue(actual["conservation"]["payer_identity"])
            self.assertTrue(actual["conservation"]["hoard_untouched"])
            self.assertEqual(
                actual["buyer_debit_total"] - actual["seller_credit_total"],
                actual["fee_pot"],
            )
        self.assertGreaterEqual(checked, 6)

    def test_terminal_ceil_vectors_pay_the_exact_ceiling_per_instance(self) -> None:
        for vector in FEE_VECTORS:
            if vector.get("carry_close") != "terminal_ceil":
                continue
            actual = replay_fee_vector(vector)
            instances = len(actual["domain_paid"])
            exact = Fraction(
                int(vector["expected"]["fee_numerator_total"]),
                int(vector["expected"]["denominator"]),
            )
            self.assertGreaterEqual(actual["fee_pot"], exact)
            self.assertLessEqual(actual["fee_pot"], exact + instances)


class FixtureContract(unittest.TestCase):
    def test_committed_files_match_canonical_bytes(self) -> None:
        directory = fixture_directory()
        for name, builder in FIXTURE_FILES.items():
            path = directory / name
            with self.subTest(fixture=name):
                self.assertTrue(path.exists(), msg=f"missing fixture {path}")
                self.assertEqual(path.read_bytes(), canonical_bytes(builder()))

    def test_serialization_is_deterministic_and_timestamp_free(self) -> None:
        for builder in (build_admission_fixture, build_trace_fixture, build_fee_fixture):
            first = canonical_bytes(builder())
            second = canonical_bytes(builder())
            self.assertEqual(first, second)
            text = first.decode("utf-8")
            for forbidden in ("timestamp", "generated_at", "/Users/", "random"):
                self.assertNotIn(forbidden, text)
            self.assertTrue(text.endswith("\n"))
            json.loads(text)

    def test_every_named_error_class_is_in_the_shared_vocabulary(self) -> None:
        found = set()
        for vector in ADMISSION_VECTORS:
            for expectation in vector["expected"].values():
                if expectation["result"] == "refuse":
                    found.add(expectation["error_class"])
        for vector in TRACE_VECTORS:
            for arm in vector["arms"].values():
                if not arm["admitted"]:
                    found.add(arm["error_class"])
                    continue
                for step in arm["results"]:
                    if step["result"] == "refuse":
                        found.add(step["error_class"])
        self.assertTrue(found.issubset(set(ERROR_CLASSES)), msg=sorted(found))
        self.assertGreaterEqual(len(found), 8)

    def test_vector_identifiers_are_unique(self) -> None:
        for family in (ADMISSION_VECTORS, TRACE_VECTORS, FEE_VECTORS):
            identifiers = [vector["id"] for vector in family]
            self.assertEqual(len(identifiers), len(set(identifiers)))


if __name__ == "__main__":
    unittest.main()
