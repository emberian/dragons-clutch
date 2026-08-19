# SPDX-License-Identifier: AGPL-3.0-or-later
"""Adversarial tests for fail-closed staged liveness arithmetic."""

from __future__ import annotations

import unittest

from admission_math import (
    AdmissionError,
    QuotePolicy,
    direct_work_budget_quote,
    exact_unique_labels,
    quote_route,
    require_runtime_schedule_covers_policy,
    resolution_path_quote,
    worst_fold_partition,
)


POLICY = QuotePolicy(
    headroom_numerator=5,
    headroom_denominator=4,
    rounding_quantum_cu=50_000,
    transaction_ceiling_cu=1_400_000,
    base_fee_cap_lamports=10_000,
    micro_lamports_per_cu_cap=1_000_000,
    keeper_tip_lamports=100_000,
)

WORK_RENT = 9_911_040
RESERVE_RENT = 890_880


class AdmissionMathTests(unittest.TestCase):
    def test_exact_headroom_boundary_and_plus_one(self) -> None:
        passing = quote_route(1_120_000, POLICY)
        self.assertEqual(passing.status, "PASS")
        self.assertEqual(passing.selected_limit_cu, 1_400_000)
        self.assertEqual(passing.keeper_reward_lamports, 1_510_000)

        stopped = quote_route(1_120_001, POLICY)
        self.assertEqual(stopped.status, "STOP_HEADROOM")
        self.assertIsNone(stopped.selected_limit_cu)
        self.assertIsNone(stopped.external_fee_cap_lamports)
        self.assertIsNone(stopped.keeper_reward_lamports)
        with self.assertRaises(AdmissionError):
            stopped.require_reward()

    def test_build_11_fold_widths_share_one_flat_reward(self) -> None:
        observed = {1: 802_103, 2: 812_043, 3: 812_983, 4: 815_423}
        quotes = {width: quote_route(cu, POLICY) for width, cu in observed.items()}
        self.assertEqual(
            {width: quote.require_reward() for width, quote in quotes.items()},
            {1: 1_160_000, 2: 1_160_000, 3: 1_160_000, 4: 1_160_000},
        )
        cost, partition = worst_fold_partition(32, quotes)
        self.assertEqual(cost, 37_120_000)
        self.assertEqual(partition, (1,) * 32)

    def test_finalize_stop_propagates_to_whole_path(self) -> None:
        folds = {
            width: quote_route(cu, POLICY)
            for width, cu in {1: 802_103, 2: 812_043, 3: 812_983, 4: 815_423}.items()
        }
        result = resolution_path_quote(
            record_count=32,
            begin=quote_route(810_842, POLICY),
            fold_quotes=folds,
            finalize=quote_route(1_170_549, POLICY),
            abort=quote_route(587_047, POLICY),
            rent_principal_lamports=WORK_RENT + RESERVE_RENT,
        )
        self.assertEqual(result.status, "STOP_FINALIZE")
        self.assertIsNone(result.spendable_reserve_lamports)
        self.assertIsNone(result.persistent_reserve_lamports)
        self.assertIsNone(result.payer_cold_outlay_lamports)

    def test_max32_policy_cap_and_abort_alternative(self) -> None:
        folds = {
            width: quote_route(cu, POLICY)
            for width, cu in {1: 802_103, 2: 812_043, 3: 812_983, 4: 815_423}.items()
        }
        result = resolution_path_quote(
            record_count=32,
            begin=quote_route(810_842, POLICY),
            fold_quotes=folds,
            # Exact largest sample that can meet the selected 25% gate.
            finalize=quote_route(1_120_000, POLICY),
            abort=quote_route(587_047, POLICY),
            rent_principal_lamports=WORK_RENT + RESERVE_RENT,
        )
        self.assertEqual(result.status, "PASS")
        self.assertEqual(result.success_rewards_lamports, 38_630_000)
        self.assertEqual(result.worst_abort_rewards_lamports, 37_980_000)
        self.assertEqual(result.spendable_reserve_lamports, 38_630_000)
        self.assertEqual(result.persistent_reserve_lamports, 49_431_920)
        self.assertEqual(result.begin_external_lamports, 1_160_000)
        self.assertEqual(result.payer_cold_outlay_lamports, 50_591_920)

    def test_runtime_schedule_must_cover_every_width_and_terminal(self) -> None:
        folds = {
            width: quote_route(cu, POLICY)
            for width, cu in {1: 802_103, 2: 812_043, 3: 812_983, 4: 815_423}.items()
        }
        finalize = quote_route(1_120_000, POLICY)
        abort = quote_route(587_047, POLICY)
        require_runtime_schedule_covers_policy(
            fold_quotes=folds,
            fold_base_reward=1_160_000,
            fold_per_record_reward=0,
            finalize_quote=finalize,
            finalize_reward=1_510_000,
            abort_quote=abort,
            abort_reward=860_000,
        )
        with self.assertRaises(AdmissionError):
            require_runtime_schedule_covers_policy(
                fold_quotes=folds,
                fold_base_reward=1_159_999,
                fold_per_record_reward=0,
                finalize_quote=finalize,
                finalize_reward=1_510_000,
                abort_quote=abort,
                abort_reward=860_000,
            )

    def test_stopped_fold_width_refuses_every_partition(self) -> None:
        folds = {
            1: quote_route(800_000, POLICY),
            2: quote_route(800_000, POLICY),
            3: quote_route(800_000, POLICY),
            4: quote_route(1_120_001, POLICY),
        }
        with self.assertRaises(AdmissionError):
            worst_fold_partition(3, folds)

    def test_shape_labels_are_exact_unique_and_ordered(self) -> None:
        exact_unique_labels([1, 2, 3], [1, 2, 3], "degree")
        for bad in ([1, 1, 3], [1, 3, 2], [1, 2], [1, 2, 3, 4]):
            with self.assertRaises(AdmissionError):
                exact_unique_labels(bad, [1, 2, 3], "degree")

    def test_direct_budget_is_a_path_max_not_a_per_order_split(self) -> None:
        result = direct_work_budget_quote(
            max_candidates=3,
            begin=quote_route(500_000, POLICY),
            verify=quote_route(600_000, POLICY),
            finalize=quote_route(700_000, POLICY),
            settle=quote_route(800_000, POLICY),
            lapse=quote_route(400_000, POLICY),
            rent_principal_lamports=2_000_000,
        )
        self.assertEqual(result.status, "PASS")
        # Quotes are 760k, 860k, 1.01m, 1.11m, and 610k respectively.
        self.assertEqual(result.selected_success_rewards_lamports, 5_460_000)
        self.assertEqual(result.selected_lapse_rewards_lamports, 4_960_000)
        self.assertEqual(result.empty_lapse_rewards_lamports, 610_000)
        self.assertEqual(result.spendable_reserve_lamports, 5_460_000)
        self.assertEqual(result.persistent_budget_lamports, 7_460_000)

    def test_direct_budget_propagates_any_mandatory_route_stop(self) -> None:
        result = direct_work_budget_quote(
            max_candidates=3,
            begin=quote_route(500_000, POLICY),
            verify=quote_route(600_000, POLICY),
            finalize=quote_route(700_000, POLICY),
            settle=quote_route(1_120_001, POLICY),
            lapse=quote_route(400_000, POLICY),
            rent_principal_lamports=2_000_000,
        )
        self.assertEqual(result.status, "STOP_SETTLE")
        self.assertIsNone(result.spendable_reserve_lamports)
        self.assertIsNone(result.persistent_budget_lamports)

    def test_direct_budget_rejects_unmeasured_shape_and_u64_overflow(self) -> None:
        with self.assertRaises(AdmissionError):
            direct_work_budget_quote(
                max_candidates=4,
                begin=quote_route(1, POLICY),
                verify=quote_route(1, POLICY),
                finalize=quote_route(1, POLICY),
                settle=quote_route(1, POLICY),
                lapse=quote_route(1, POLICY),
                rent_principal_lamports=1,
            )
        with self.assertRaises(AdmissionError):
            direct_work_budget_quote(
                max_candidates=3,
                begin=quote_route(1, POLICY),
                verify=quote_route(1, POLICY),
                finalize=quote_route(1, POLICY),
                settle=quote_route(1, POLICY),
                lapse=quote_route(1, POLICY),
                rent_principal_lamports=(1 << 64) - 1,
            )


if __name__ == "__main__":
    unittest.main()
