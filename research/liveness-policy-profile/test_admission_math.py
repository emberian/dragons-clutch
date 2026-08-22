# SPDX-License-Identifier: AGPL-3.0-or-later
"""Adversarial tests for fail-closed staged liveness arithmetic."""

from __future__ import annotations

import unittest

from admission_math import (
    AdmissionError,
    QuotePolicy,
    RuntimeCostSchedule,
    batched_external_resolution_budget_quote,
    direct_work_budget_quote,
    exact_unique_labels,
    external_resolution_budget_quote,
    fewest_transaction_batch_plan,
    protocol_resolution_prefund,
    quote_route,
    require_runtime_schedule_covers_batches,
    require_runtime_schedule_covers_policy,
    runtime_execution_plan,
    worst_fold_partition,
)


POLICY = QuotePolicy(
    headroom_numerator=5,
    headroom_denominator=4,
    rounding_quantum_cu=10_000,
    transaction_ceiling_cu=1_400_000,
    base_fee_cap_lamports=10_000,
    micro_lamports_per_cu_cap=1_000_000,
    keeper_tip_lamports=100_000,
)

WORK_RENT = 9_911_040
RESERVE_RENT = 890_880

RUNTIME_SCHEDULE = RuntimeCostSchedule(
    maximum_records=32,
    maximum_fold_width=4,
    begin_charge_lamports=0,
    fold_base_charge_lamports=0,
    fold_per_record_charge_lamports=0,
    fold_base_reward_lamports=1_160_000,
    fold_per_record_reward_lamports=0,
    finalize_charge_lamports=0,
    finalize_reward_lamports=1_510_000,
    abort_charge_lamports=0,
    abort_reward_lamports=860_000,
)


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

    def test_build_14_fold_widths_quote_exactly_under_finer_quantum(self) -> None:
        observed = {1: 804_616, 2: 812_193, 3: 813_128, 4: 815_573}
        quotes = {width: quote_route(cu, POLICY) for width, cu in observed.items()}
        self.assertEqual(
            {width: quote.require_reward() for width, quote in quotes.items()},
            {1: 1_120_000, 2: 1_130_000, 3: 1_130_000, 4: 1_130_000},
        )
        cost, partition = worst_fold_partition(32, quotes)
        self.assertEqual(cost, 35_840_000)
        self.assertEqual(partition, (1,) * 32)

    def test_finalize_stop_propagates_to_whole_path(self) -> None:
        folds = {
            width: quote_route(cu, POLICY)
            for width, cu in {1: 804_616, 2: 812_193, 3: 813_128, 4: 815_573}.items()
        }
        result = external_resolution_budget_quote(
            record_count=32,
            begin=quote_route(810_992, POLICY),
            fold_quotes=folds,
            finalize=quote_route(1_170_549, POLICY),
            abort=quote_route(587_047, POLICY),
        )
        self.assertEqual(result.status, "STOP_FINALIZE")
        self.assertIsNone(result.success_post_begin_budget_lamports)
        self.assertIsNone(result.success_total_budget_lamports)

    def test_max32_policy_cap_and_abort_alternative(self) -> None:
        folds = {
            width: quote_route(cu, POLICY)
            for width, cu in {1: 804_616, 2: 812_193, 3: 813_128, 4: 815_573}.items()
        }
        result = external_resolution_budget_quote(
            record_count=32,
            begin=quote_route(810_992, POLICY),
            fold_quotes=folds,
            finalize=quote_route(1_094_832, POLICY),
            abort=quote_route(587_197, POLICY),
        )
        self.assertEqual(result.status, "PASS")
        self.assertEqual(result.success_post_begin_budget_lamports, 37_320_000)
        self.assertEqual(result.worst_abort_post_begin_budget_lamports, 36_690_000)
        self.assertEqual(result.begin_transaction_budget_lamports, 1_130_000)
        self.assertEqual(result.success_total_budget_lamports, 38_450_000)
        self.assertEqual(result.worst_abort_total_budget_lamports, 37_820_000)
        self.assertFalse(hasattr(result, "rent_principal_lamports"))

    def test_runtime_schedule_must_cover_every_width_and_terminal(self) -> None:
        folds = {
            width: quote_route(cu, POLICY)
            for width, cu in {1: 804_616, 2: 812_193, 3: 813_128, 4: 815_573}.items()
        }
        finalize = quote_route(1_094_832, POLICY)
        abort = quote_route(587_197, POLICY)
        require_runtime_schedule_covers_policy(
            fold_quotes=folds,
            fold_base_reward=1_160_000,
            fold_per_record_reward=0,
            finalize_quote=finalize,
            finalize_reward=1_510_000,
            abort_quote=abort,
            abort_reward=860_000,
        )
        # The widest fold quotes 1,130,000 under the 10,000-CU quantum, so
        # one lamport below that exact boundary must be refused.
        with self.assertRaises(AdmissionError):
            require_runtime_schedule_covers_policy(
                fold_quotes=folds,
                fold_base_reward=1_129_999,
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
        # Quotes are 740k, 860k, 990k, 1.11m, and 610k respectively.
        self.assertEqual(result.selected_success_rewards_lamports, 5_420_000)
        self.assertEqual(result.unselected_lapse_rewards_lamports, 3_930_000)
        self.assertEqual(result.selected_lapse_rewards_lamports, 4_920_000)
        self.assertEqual(result.empty_lapse_rewards_lamports, 610_000)
        self.assertEqual(result.spendable_reserve_lamports, 5_420_000)
        self.assertEqual(result.persistent_budget_lamports, 7_420_000)

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

    def test_fewest_transaction_batch_plan_is_minimal_and_fail_closed(self) -> None:
        measured = {1: 88_217, 2: 175_781, 4: 331_077, 8: 664_117, 12: 926_969}
        quotes = {size: quote_route(cu, POLICY) for size, cu in measured.items()}
        self.assertEqual(fewest_transaction_batch_plan(32, quotes), (12, 12, 8))
        self.assertEqual(fewest_transaction_batch_plan(1, quotes), (1,))
        self.assertEqual(fewest_transaction_batch_plan(7, quotes), (4, 2, 1))
        # A stopped batch size drops out of the search instead of being
        # clamped into a price.
        stopped_12 = dict(quotes)
        stopped_12[12] = quote_route(1_120_001, POLICY)
        self.assertFalse(stopped_12[12].admitted)
        self.assertEqual(fewest_transaction_batch_plan(32, stopped_12), (8, 8, 8, 8))
        # A record count no admitted batch combination reaches is refused.
        evens = {size: quotes[size] for size in (2, 4)}
        with self.assertRaises(AdmissionError):
            fewest_transaction_batch_plan(5, evens)
        with self.assertRaises(AdmissionError):
            fewest_transaction_batch_plan(32, {2: quote_route(1_120_001, POLICY)})
        with self.assertRaises(AdmissionError):
            fewest_transaction_batch_plan(0, quotes)

    def test_batched_path_prices_measured_plan_and_propagates_stops(self) -> None:
        measured = {1: 88_217, 2: 175_781, 4: 331_077, 8: 664_117, 12: 926_969}
        quotes = {size: quote_route(cu, POLICY) for size, cu in measured.items()}
        begin = quote_route(90_924, POLICY)
        finalize = quote_route(164_287, POLICY)
        abort = quote_route(46_677, POLICY)
        result = batched_external_resolution_budget_quote(
            record_count=32,
            begin=begin,
            batch_quotes=quotes,
            finalize=finalize,
            abort=abort,
        )
        self.assertEqual(result.status, "PASS")
        self.assertEqual(result.transaction_plan, (12, 12, 8))
        self.assertEqual(result.fold_transactions, 3)
        self.assertEqual(result.fold_transactions_budget_lamports, 3_490_000)
        self.assertEqual(result.success_post_begin_budget_lamports, 3_810_000)
        self.assertEqual(result.worst_abort_post_begin_budget_lamports, 3_660_000)
        self.assertEqual(result.begin_transaction_budget_lamports, 230_000)
        self.assertEqual(result.success_total_budget_lamports, 4_040_000)
        self.assertEqual(result.worst_abort_total_budget_lamports, 3_890_000)
        self.assertFalse(hasattr(result, "rent_principal_lamports"))

        stopped_finalize = batched_external_resolution_budget_quote(
            record_count=32,
            begin=begin,
            batch_quotes=quotes,
            finalize=quote_route(1_120_001, POLICY),
            abort=abort,
        )
        self.assertEqual(stopped_finalize.status, "STOP_FINALIZE")
        self.assertIsNone(stopped_finalize.transaction_plan)
        self.assertIsNone(stopped_finalize.success_total_budget_lamports)

        all_stopped = {size: quote_route(1_120_001, POLICY) for size in quotes}
        no_batches = batched_external_resolution_budget_quote(
            record_count=32,
            begin=begin,
            batch_quotes=all_stopped,
            finalize=finalize,
            abort=abort,
        )
        self.assertEqual(no_batches.status, "STOP_FOLD_BATCH")
        self.assertIsNone(no_batches.fold_transactions_budget_lamports)
        self.assertIsNone(no_batches.success_total_budget_lamports)

    def test_runtime_schedule_must_cover_every_admitted_batch(self) -> None:
        measured = {2: 175_781, 4: 331_077, 8: 664_117, 12: 926_969}
        quotes = {size: quote_route(cu, POLICY) for size, cu in measured.items()}
        # The two-fold batch quotes 330,000, so 165,000 per fold covers every
        # measured batch and one lamport below that boundary must be refused.
        require_runtime_schedule_covers_batches(
            batch_quotes=quotes,
            fold_base_reward=165_000,
            fold_per_record_reward=0,
        )
        with self.assertRaises(AdmissionError):
            require_runtime_schedule_covers_batches(
                batch_quotes=quotes,
                fold_base_reward=164_999,
                fold_per_record_reward=0,
            )
        # A stopped batch size carries no quote to cover and is skipped.
        stopped = dict(quotes)
        stopped[12] = quote_route(1_120_001, POLICY)
        require_runtime_schedule_covers_batches(
            batch_quotes=stopped,
            fold_base_reward=165_000,
            fold_per_record_reward=0,
        )

    def test_protocol_prefund_uses_worst_case_successful_fold_call_count(self) -> None:
        prefund = protocol_resolution_prefund(
            record_count=32,
            rent_principal_lamports=WORK_RENT + RESERVE_RENT,
            schedule=RUNTIME_SCHEDULE,
        )
        self.assertEqual(prefund.worst_case_fold_calls, 32)
        self.assertEqual(prefund.worst_case_fold_outflow_lamports, 37_120_000)
        self.assertEqual(prefund.terminal_outflow_lamports, 1_510_000)
        self.assertEqual(prefund.spendable_reserve_lamports, 38_630_000)
        self.assertEqual(prefund.rent_principal_lamports, 10_801_920)
        self.assertEqual(prefund.minimum_prefund_lamports, 49_431_920)

    def test_fold4_plan_derives_per_call_payout_and_terminal_refund(self) -> None:
        prefund = protocol_resolution_prefund(
            record_count=32,
            rent_principal_lamports=WORK_RENT + RESERVE_RENT,
            schedule=RUNTIME_SCHEDULE,
        )
        plan = runtime_execution_plan(
            name="EIGHT_FOLD4_CALLS_IN_SIX_PLUS_TWO_TRANSACTIONS",
            fold_call_widths=(4,) * 8,
            transaction_fold_call_counts=(6, 2),
            prefund=prefund,
            schedule=RUNTIME_SCHEDULE,
        )
        self.assertEqual(plan.fold_calls, 8)
        self.assertEqual(plan.fold_transactions, 2)
        self.assertEqual(plan.fold_rewards_lamports, 9_280_000)
        self.assertEqual(plan.success_payout_lamports, 10_790_000)
        self.assertEqual(plan.success_unused_prepaid_lamports, 27_840_000)
        self.assertEqual(plan.success_rent_principal_refund_lamports, 10_801_920)
        self.assertEqual(plan.success_payer_refund_lamports, 38_641_920)
        self.assertEqual(plan.abort_payout_lamports, 10_140_000)
        self.assertEqual(plan.abort_payer_refund_lamports, 39_291_920)

    def test_runtime_plan_refuses_nonpartition_and_prefund_is_batch_invariant(self) -> None:
        prefund = protocol_resolution_prefund(
            record_count=32,
            rent_principal_lamports=WORK_RENT + RESERVE_RENT,
            schedule=RUNTIME_SCHEDULE,
        )
        with self.assertRaises(AdmissionError):
            runtime_execution_plan(
                name="MISSING_ONE_RECORD",
                fold_call_widths=(4,) * 7 + (3,),
                transaction_fold_call_counts=(6, 2),
                prefund=prefund,
                schedule=RUNTIME_SCHEDULE,
            )
        with self.assertRaises(AdmissionError):
            runtime_execution_plan(
                name="MISSING_ONE_CALL_FROM_TRANSACTION_PARTITION",
                fold_call_widths=(4,) * 8,
                transaction_fold_call_counts=(6, 1),
                prefund=prefund,
                schedule=RUNTIME_SCHEDULE,
            )

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
