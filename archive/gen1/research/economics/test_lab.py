# SPDX-License-Identifier: AGPL-3.0-or-later
"""Property-oriented tests for the deterministic economics laboratory."""

from __future__ import annotations

import unittest
from fractions import Fraction
from itertools import permutations

from model import (
    ALLOWED_POOL_PURPOSES,
    CategoricalMarket,
    LivenessBook,
    LivenessJob,
    ModelError,
    Pool,
    ProtectedPools,
    ReverseDutchSchedule,
    SharedFeedEpoch,
    allocate_fee,
    common_mode_exposure,
    compatible_payout,
    dispersion_numerator,
    dominant_tail_attack,
    enumerate_solvency_traces,
    exhaustive_liveness_orders,
    exposure_admissible,
    fee_fragmentation_result,
    integer_shares,
    maximum_liability,
    midpoint_effective_bps,
    one_hot_vectors,
    portfolio_payout,
    required_weighted_volume,
    single_egg_dispersion_numerator,
    stateless_ceil_fee,
    wash_cycle_loss,
)


class SolvencyProperties(unittest.TestCase):
    def test_bounded_reachable_traces_preserve_solvency(self) -> None:
        result = enumerate_solvency_traces(outcomes=3, depth=8, hoard_cap=5)
        self.assertGreater(result["states"], 100)
        self.assertGreater(result["transitions"], result["states"])

    def test_direct_burn_is_a_donation(self) -> None:
        state = CategoricalMarket.empty(3).split(10)
        margin_before = state.hoard - state.required_collateral()
        burned = state.burn(0, 4)
        self.assertTrue(burned.is_solvent())
        self.assertGreaterEqual(burned.hoard - burned.required_collateral(), margin_before)

    def test_resolved_redemptions_preserve_local_liability(self) -> None:
        state = CategoricalMarket.empty(4).split(20).burn(2, 3).resolve(1)
        for outcome in range(4):
            state = state.redeem(outcome, 5)
            self.assertTrue(state.is_solvent())

    def test_general_payout_set_is_bounded(self) -> None:
        supplies = (10, 7, 3)
        payouts = (*one_hot_vectors(3), compatible_payout(3, (0, 2)))
        self.assertEqual(maximum_liability(supplies, payouts), 10)


class ProtectedPoolProperties(unittest.TestCase):
    def test_every_forbidden_pool_purpose_refuses_without_mutation(self) -> None:
        book = ProtectedPools(50, 50, 50, 50, 50, 50, 50)
        purposes = {purpose for allowed in ALLOWED_POOL_PURPOSES.values() for purpose in allowed}
        for pool in Pool:
            for purpose in purposes:
                if purpose in ALLOWED_POOL_PURPOSES[pool]:
                    updated = book.debit(pool, purpose, 1)
                    self.assertEqual(updated.balance(pool), 49)
                    for other in Pool:
                        if other != pool:
                            self.assertEqual(updated.balance(other), 50)
                else:
                    with self.assertRaises(ModelError):
                        book.debit(pool, purpose, 1)
                    self.assertEqual(book.as_tuple(), (50,) * 7)

    def test_hoard_cannot_pay_liveness_or_revenue(self) -> None:
        book = ProtectedPools(100)
        for purpose in ("observation", "repair", "maker_rebate", "protocol", "operations"):
            with self.assertRaises(ModelError):
                book.debit(Pool.HOARD, purpose, 1)


class LivenessProperties(unittest.TestCase):
    def test_admission_uses_maxima_not_expected_payment(self) -> None:
        book = LivenessBook(sol_balance=10, reward_balance=4)
        with self.assertRaises(ModelError):
            book.book(LivenessJob("observation", max_sol=11, max_reward=1))
        admitted = book.book(LivenessJob("observation", max_sol=10, max_reward=4))
        self.assertEqual(admitted.free_sol(), 0)
        self.assertEqual(admitted.free_reward(), 0)

    def test_completion_releases_only_unused_cap_and_cannot_repeat(self) -> None:
        book = LivenessBook(100, 50).book(LivenessJob("bucket-1", 40, 20))
        completed = book.complete("bucket-1", paid_sol=13, paid_reward=7)
        self.assertEqual(completed.sol_balance, 87)
        self.assertEqual(completed.reward_balance, 43)
        self.assertEqual(completed.free_sol(), 87)
        with self.assertRaises(ModelError):
            completed.complete("bucket-1", 1, 1)

    def test_booking_is_order_independent(self) -> None:
        result = exhaustive_liveness_orders()
        self.assertGreater(result["admitted"], 0)
        self.assertGreater(result["refused"], 0)

    def test_reverse_dutch_schedule_is_bounded_and_monotone(self) -> None:
        schedule = ReverseDutchSchedule((10, 15, 23, 40))
        self.assertEqual(schedule.booked_maximum, 40)
        self.assertEqual([schedule.offer(i) for i in range(4)], [10, 15, 23, 40])
        with self.assertRaises(ModelError):
            ReverseDutchSchedule((10, 9))


class SharedFeedProperties(unittest.TestCase):
    def test_integer_capitalization_never_drops_and_rounding_is_one_atom(self) -> None:
        for reserve in range(0, 128):
            epoch = SharedFeedEpoch.first(reserve)
            for subscriber_count in range(1, 33):
                self.assertEqual(sum(epoch.capital_shares), reserve)
                self.assertLessEqual(max(epoch.capital_shares) - min(epoch.capital_shares), 1)
                if subscriber_count < 32:
                    epoch, join = epoch.join()
                    self.assertEqual(join.deposit, sum(join.reimbursements))

    def test_success_charges_actual_cost_equally(self) -> None:
        epoch = SharedFeedEpoch.first(101)
        for _ in range(7):
            epoch, _ = epoch.join()
        for actual in range(0, 102):
            settlement = epoch.settle(actual, success=True)
            self.assertEqual(sum(settlement.subscriber_costs), actual)
            self.assertEqual(sum(settlement.subscriber_refunds), 101 - actual)
            self.assertLessEqual(
                max(settlement.subscriber_costs) - min(settlement.subscriber_costs), 1
            )
            self.assertEqual(settlement.neutral_reserve_roll, 0)

    def test_failure_does_not_refund_interested_subscribers(self) -> None:
        epoch = SharedFeedEpoch.first(101)
        for _ in range(3):
            epoch, _ = epoch.join()
        settlement = epoch.settle(37, success=False)
        self.assertEqual(settlement.subscriber_refunds, (0, 0, 0, 0))
        self.assertEqual(settlement.neutral_reserve_roll, 64)
        self.assertEqual(sum(settlement.subscriber_costs), 101)

    def test_integer_share_rule_is_canonical(self) -> None:
        self.assertEqual(integer_shares(10, 3), (4, 3, 3))
        self.assertEqual(integer_shares(2, 4), (1, 1, 0, 0))


class FailureAttackProperties(unittest.TestCase):
    def test_equal_failure_tail_basket_approaches_full_hoard(self) -> None:
        attack = dominant_tail_attack(16, 1_000_000, Fraction(1, 100))
        self.assertEqual(attack["fallback_payoff"], Fraction(15_000_000, 16))
        self.assertGreater(attack["net_gain"], Fraction(9, 10) * 1_000_000)

    def test_compatible_set_removes_incompatible_tail_payout(self) -> None:
        payout = compatible_payout(4, (0, 2))
        self.assertEqual(portfolio_payout((0, 0, 0, 10), payout), 0)
        self.assertEqual(portfolio_payout((0, 0, 10, 0), payout), 5)

    def test_narrow_compatibility_can_raise_one_remaining_tail_weight(self) -> None:
        equal = compatible_payout(4, (0, 1, 2, 3))
        narrow = compatible_payout(4, (0, 3))
        holdings = (0, 0, 0, 100)
        self.assertGreater(portfolio_payout(holdings, narrow), portfolio_payout(holdings, equal))

    def test_common_mode_cap_aggregates_markets(self) -> None:
        exposure = common_mode_exposure(
            (1_000, 2_000, 4_000),
            (Fraction(1), Fraction(1, 2), Fraction(1, 4)),
        )
        self.assertEqual(exposure, 3_000)
        self.assertTrue(exposure_admissible(exposure, Fraction(30_000)))
        self.assertFalse(exposure_admissible(exposure + 1, Fraction(30_000)))


class FeeProperties(unittest.TestCase):
    SCALE = 10_000

    def test_midpoint_hypothesis_is_twenty_basis_points(self) -> None:
        self.assertEqual(midpoint_effective_bps(Fraction(4, 1_000)), 20)

    def test_persistent_carry_is_fragmentation_invariant(self) -> None:
        for total in range(1, 80):
            fragments = (1,) * total
            result = fee_fragmentation_result(fragments, 3_333, self.SCALE, 4, 1_000)
            self.assertEqual(result["persistent_total"], result["whole_floor"])
            self.assertEqual(result["persistent_carry"], result["whole_carry"])

    def test_resetting_carry_can_erase_dust_fee(self) -> None:
        result = fee_fragmentation_result((1,) * 1_001, 5_000, self.SCALE, 4, 1_000)
        self.assertLess(result["reset_total"], result["whole_floor"])
        self.assertEqual(result["persistent_total"], result["whole_floor"])

    def test_stateless_ceil_splitting_cannot_reduce_fee(self) -> None:
        for left in range(0, 40):
            for right in range(0, 40):
                left_fee = stateless_ceil_fee(
                    single_egg_dispersion_numerator(left, 2_500, self.SCALE),
                    self.SCALE,
                    4,
                    1_000,
                )
                right_fee = stateless_ceil_fee(
                    single_egg_dispersion_numerator(right, 2_500, self.SCALE),
                    self.SCALE,
                    4,
                    1_000,
                )
                whole_fee = stateless_ceil_fee(
                    single_egg_dispersion_numerator(left + right, 2_500, self.SCALE),
                    self.SCALE,
                    4,
                    1_000,
                )
                self.assertGreaterEqual(left_fee + right_fee, whole_fee)

    def test_dispersion_complete_set_and_relabeling_invariance(self) -> None:
        payoffs = (2, 7, 4, 11)
        prices = (1_000, 2_000, 3_000, 4_000)
        base = dispersion_numerator(payoffs, prices)
        shifted = dispersion_numerator(tuple(value + 19 for value in payoffs), prices)
        self.assertEqual(base, shifted)
        for order in permutations(range(4)):
            self.assertEqual(
                base,
                dispersion_numerator(
                    tuple(payoffs[index] for index in order),
                    tuple(prices[index] for index in order),
                ),
            )

    def test_dispersion_partition_refinement_invariance(self) -> None:
        original = dispersion_numerator((5, 1), (3_000, 7_000))
        refined = dispersion_numerator((5, 5, 1), (1_000, 2_000, 7_000))
        self.assertEqual(original, refined)

    def test_allocations_conserve_and_wash_has_treasury_floor(self) -> None:
        for fee in range(0, 10_001):
            # PROPOSED variant, explicitly named (P0-5)
            allocation = allocate_fee(
                fee,
                maker_num=60,
                executor_num=15,
                denominator=100,
                executor_cap=None,
            )
            self.assertEqual(allocation.total, fee)
            self.assertGreaterEqual(allocation.treasury * 100, fee * 25)
            self.assertEqual(
                wash_cycle_loss(
                    fee,
                    maker_num=60,
                    executor_num=15,
                    denominator=100,
                    executor_cap=None,
                ),
                allocation.treasury,
            )
            self.assertGreaterEqual(
                wash_cycle_loss(
                    fee,
                    maker_num=60,
                    executor_num=15,
                    denominator=100,
                    executor_cap=None,
                    network_cost=7,
                ),
                allocation.treasury + 7,
            )


class PriceAndRevenueProperties(unittest.TestCase):
    def test_dregg_price_zero_makes_fee_break_even_unbounded(self) -> None:
        required = required_weighted_volume(
            Fraction(100), Fraction(10), Fraction(4, 1_000), Fraction(1, 4), Fraction(0)
        )
        self.assertIsNone(required)

    def test_prepaid_service_premium_can_cover_cost_without_volume(self) -> None:
        required = required_weighted_volume(
            Fraction(100), Fraction(100), Fraction(0), Fraction(0), Fraction(0)
        )
        self.assertEqual(required, 0)

    def test_break_even_volume_is_monotone_in_price_and_fee(self) -> None:
        low_price = required_weighted_volume(
            Fraction(100), Fraction(10), Fraction(4, 1_000), Fraction(1, 4), Fraction(1, 10_000_000)
        )
        high_price = required_weighted_volume(
            Fraction(100), Fraction(10), Fraction(4, 1_000), Fraction(1, 4), Fraction(1, 1_000_000)
        )
        high_fee = required_weighted_volume(
            Fraction(100), Fraction(10), Fraction(1, 100), Fraction(1, 4), Fraction(1, 1_000_000)
        )
        assert low_price is not None and high_price is not None and high_fee is not None
        self.assertGreater(low_price, high_price)
        self.assertGreater(high_price, high_fee)


if __name__ == "__main__":
    unittest.main()
