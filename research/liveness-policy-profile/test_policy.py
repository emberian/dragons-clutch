# SPDX-License-Identifier: AGPL-3.0-or-later
"""Adversarial arithmetic and capture-parser checks for policy.py."""

from __future__ import annotations

import copy
import unittest

import policy


class PolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.evidence = policy.load_evidence()

    def test_checked_in_projection_is_exactly_rederived(self) -> None:
        self.assertEqual(policy.derive(self.evidence), self.evidence["projection"])

    def test_cu_envelope_is_monotone_and_never_exceeds_ceiling(self) -> None:
        inputs = self.evidence["policy_inputs"]
        previous = 0
        for consumed in range(0, 1_400_001, 997):
            envelope, _ = policy.cu_envelope(consumed, inputs)
            self.assertGreaterEqual(envelope, previous)
            self.assertLessEqual(envelope, inputs["transaction_cu_ceiling"])
            previous = envelope

    def test_ceiling_refuses_to_masquerade_as_requested_headroom(self) -> None:
        inputs = self.evidence["policy_inputs"]
        submit = self.evidence["measurements"]["candidate.submit_direct_page"]["sample_cu"]
        resolve = max(
            self.evidence["measurements"]["resolution.native.resolve"]["samples_cu"]
        )
        self.assertEqual(policy.cu_envelope(submit, inputs), (1_400_000, False))
        self.assertEqual(policy.cu_envelope(resolve, inputs), (1_400_000, False))

    def test_one_cu_above_exact_gate_is_not_accepted(self) -> None:
        inputs = self.evidence["policy_inputs"]
        largest_with_25_percent_headroom = (
            inputs["transaction_cu_ceiling"]
            * inputs["cu_headroom_denominator"]
            // inputs["cu_headroom_numerator"]
        )
        self.assertTrue(policy.cu_envelope(largest_with_25_percent_headroom, inputs)[1])
        self.assertFalse(policy.cu_envelope(largest_with_25_percent_headroom + 1, inputs)[1])

    def test_capture_parser_accounts_for_refusals_without_pricing_them(self) -> None:
        text = (policy.PROFILE_DIR / "captured-output.txt").read_text(encoding="utf-8")
        parsed = policy.capture_measurements(text)
        self.assertEqual(parsed, self.evidence["measurements"])
        # The early-seal and duplicate-write refusal CUs are present in the raw
        # trace but are not capitalized as mandatory successful work.
        self.assertNotIn(
            12_760,
            parsed["artifact.terms"]["successful_transactions_cu"],
        )
        self.assertNotIn(
            20_330,
            parsed["artifact.terms"]["successful_transactions_cu"],
        )

    def test_capture_parser_rejects_a_missing_native_degree(self) -> None:
        text = (policy.PROFILE_DIR / "captured-output.txt").read_text(encoding="utf-8")
        tampered = text.replace(
            "d3 resolve=1165736 retry=1012088 redeem_internal=705473\n", ""
        )
        with self.assertRaises(policy.CheckError):
            policy.capture_measurements(tampered)

    def test_rent_formula_detects_one_lamport_drift(self) -> None:
        tampered = copy.deepcopy(self.evidence)
        tampered["accounts"]["source.archive"]["rent_lamports"] += 1
        with self.assertRaises(policy.CheckError):
            policy.check_rent(tampered)

    def test_projection_has_no_shared_feed_number_without_cu_evidence(self) -> None:
        projection = policy.derive(self.evidence)
        self.assertEqual(projection["shared_feed_pair"], "INCOMPLETE_UNMEASURED")

    def test_policy_has_no_price_or_future_volume_input(self) -> None:
        keys = set(self.evidence["policy_inputs"])
        forbidden = {
            "sol_price",
            "token_price",
            "future_volume",
            "future_fees",
            "hoard_lamports",
            "hoard_collateral",
        }
        self.assertTrue(keys.isdisjoint(forbidden))


if __name__ == "__main__":
    unittest.main()
