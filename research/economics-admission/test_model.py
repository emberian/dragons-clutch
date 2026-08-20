# SPDX-License-Identifier: AGPL-3.0-or-later
"""Bounded adversarial checks for the exact economics-admission model."""

from __future__ import annotations

import itertools
import unittest

from model import (
    AdmissionFunding,
    CollateralLedger,
    EndowmentBook,
    FeeBasis,
    FeePolicy,
    JobPayment,
    MaintainerCashflow,
    MandatoryJob,
    ModelError,
    SharedFeedReserve,
    admit_market,
    allocate_fee,
    ceil_div,
    dispersion_base,
    fee_quote,
    fee_sequence,
    flat_cash_base,
    maximum_binary_single_egg_fee,
    per_egg_leg_base,
    quote_admission,
    quotient_range_base,
    split_identical_cell,
    wash_result,
)


class ProtectedCollateralTests(unittest.TestCase):
    def test_claim_principal_is_unchanged_by_every_order_and_fee_leg(self) -> None:
        initial = CollateralLedger(claim_collateral=700, free_user_cash=400)
        state = initial.reserve_order(200, 25)
        state = state.settle_reserved_buy(130, 11)
        state = state.release_order(70, 14)
        state = state.allocate_fees(60, 15, 100, executor_cap=2)
        self.assertEqual(state.claim_collateral, initial.claim_collateral)
        self.assertEqual(state.total_atoms, initial.total_atoms)
        self.assertEqual(state.reserved_consideration, 0)
        self.assertEqual(state.reserved_fee, 0)

    def test_claim_lock_is_a_conserving_reclassification(self) -> None:
        state = CollateralLedger(free_user_cash=19).lock_claim_collateral(13)
        self.assertEqual(state.claim_collateral, 13)
        self.assertEqual(state.free_user_cash, 6)
        self.assertEqual(state.total_atoms, 19)

    def test_order_cannot_borrow_claim_principal_or_another_fee_reserve(self) -> None:
        with self.assertRaises(ModelError):
            CollateralLedger(claim_collateral=1_000).reserve_order(1, 0)
        state = CollateralLedger(free_user_cash=10).reserve_order(5, 5)
        with self.assertRaises(ModelError):
            state.settle_reserved_buy(5, 6)

    def test_fee_allocation_conserves_every_atom_and_caps_executor(self) -> None:
        for pot in range(101):
            for cap in (0, 1, 7, 100):
                allocation = allocate_fee(pot, 60, 15, 100, cap)
                self.assertEqual(allocation.total, pot)
                self.assertLessEqual(allocation.executor, cap)
                self.assertGreaterEqual(allocation.treasury * 4, pot)

    def test_collateral_revenue_is_not_implicitly_sold_to_cover_sol_cost(self) -> None:
        report = MaintainerCashflow(
            treasury_collateral_atoms=10**30,
            direct_sol_revenue_lamports=99,
            measured_sol_cost_lamports=100,
        )
        self.assertFalse(report.direct_sol_break_even)
        self.assertEqual(report.direct_sol_surplus_lamports, -1)


class AdmissionTests(unittest.TestCase):
    JOBS = (
        MandatoryJob("observe", 7, 2, 3),
        MandatoryJob("repair", 11, 5, 4),
        MandatoryJob("finalize", 13, 1, 0),
    )

    def test_quote_is_sum_of_worst_case_remaining_jobs(self) -> None:
        feed = SharedFeedReserve.empty(17)
        quote = quote_admission(self.JOBS, feed)
        self.assertEqual(quote.work_lamports, 31)
        self.assertEqual(quote.storage_lamports, 8)
        self.assertEqual(quote.service_atoms, 7)
        self.assertEqual(quote.feed_join_lamports, 17)

    def test_each_underfunded_asset_refuses_independently(self) -> None:
        feed = SharedFeedReserve.empty(17)
        exact = AdmissionFunding(31, 8, "DREGG", 7, 17)
        for field in (
            "work_lamports",
            "storage_lamports",
            "service_atoms",
            "feed_join_lamports",
        ):
            values = dict(exact.__dict__)
            values[field] -= 1
            with self.subTest(field=field), self.assertRaises(ModelError):
                admit_market("m", self.JOBS, feed, AdmissionFunding(**values))

    def test_reward_price_and_treasury_cannot_substitute_for_missing_sol(self) -> None:
        # External prices and a large collateral treasury are intentionally not
        # admission inputs.  Missing one lamport refuses at every imagined price.
        huge_unrelated = CollateralLedger(treasury_revenue=10**30)
        self.assertGreater(huge_unrelated.treasury_revenue, 0)
        for imagined_reward_price in (0, 1, 10**30):
            self.assertGreaterEqual(imagined_reward_price, 0)
            with self.assertRaises(ModelError):
                admit_market(
                    "m",
                    self.JOBS,
                    SharedFeedReserve.empty(17),
                    AdmissionFunding(30, 8, "DREGG", 10**30, 17),
                )

    def test_reward_asset_is_generic_and_price_free(self) -> None:
        for asset in ("DREGG", "OTHER-SPL", "SERVICE-POINT"):
            admitted = admit_market(
                asset,
                self.JOBS,
                SharedFeedReserve.empty(17),
                AdmissionFunding(31, 8, asset, 7, 17),
            )
            self.assertEqual(admitted.endowment.service_asset, asset)
            self.assertEqual(admitted.endowment.booked_service_atoms, 7)

    def test_remaining_jobs_stay_fully_funded_after_any_completion_order(self) -> None:
        for order in itertools.permutations(self.JOBS):
            book = EndowmentBook.admit(self.JOBS, 31, 8, "reward", 7)
            for job in order:
                payment = JobPayment(
                    job.max_work_lamports // 2,
                    job.max_storage_lamports // 2,
                    job.max_service_atoms // 2,
                )
                book = book.complete(job.job_id, payment)
                self.assertGreaterEqual(book.free_work_lamports, 0)
                self.assertGreaterEqual(book.free_storage_lamports, 0)
                self.assertGreaterEqual(book.free_service_atoms, 0)

    def test_new_order_job_must_bring_its_remaining_work(self) -> None:
        book = EndowmentBook.admit((), 0, 0, None, 0)
        job = MandatoryJob("settle-order-1", 9, 4, 0)
        with self.assertRaises(ModelError):
            book.add_job(job)
        funded = book.add_job(job, work_deposit=9, storage_deposit=4)
        self.assertEqual(funded.free_work_lamports, 0)
        self.assertEqual(funded.free_storage_lamports, 0)

    def test_job_cannot_spend_more_than_its_frozen_maximum(self) -> None:
        book = EndowmentBook.admit(self.JOBS, 31, 8, "reward", 7)
        with self.assertRaises(ModelError):
            book.complete("observe", JobPayment(8, 0, 0))
        with self.assertRaises(ModelError):
            book.complete("observe", JobPayment(0, 3, 0))
        with self.assertRaises(ModelError):
            book.complete("observe", JobPayment(0, 0, 4))


class SharedFeedTests(unittest.TestCase):
    def test_join_and_refund_identities_exhaustively(self) -> None:
        for cap in range(0, 33):
            feed = SharedFeedReserve.empty(cap)
            net_paid: dict[str, int] = {}
            for index in range(1, 17):
                subscriber = f"m{index}"
                deposit = feed.required_join_deposit()
                feed, join = feed.join(subscriber, deposit)
                net_paid[subscriber] = deposit
                for incumbent, reimbursement in join.reimbursements:
                    net_paid[incumbent] -= reimbursement
                self.assertEqual(sum(net_paid.values()), cap)
                self.assertEqual(
                    tuple(net_paid[name] for name in feed.subscribers),
                    feed.capital_shares,
                )
                self.assertLessEqual(
                    max(feed.capital_shares) - min(feed.capital_shares), 1
                )
                for spend in range(cap + 1):
                    settled = feed.settle(spend, success=True)
                    costs = dict(settled.subscriber_costs)
                    refunds = dict(settled.subscriber_refunds)
                    self.assertEqual(sum(costs.values()), spend)
                    self.assertEqual(sum(refunds.values()), cap - spend)
                    for name in feed.subscribers:
                        self.assertEqual(costs[name] + refunds[name], net_paid[name])

    def test_failure_roll_is_neutral_and_never_a_creator_refund(self) -> None:
        feed = SharedFeedReserve.empty(19)
        for name in ("creator", "other", "third"):
            feed, _ = feed.join(name, feed.required_join_deposit())
        settled = feed.settle(7, success=False)
        self.assertEqual(sum(dict(settled.subscriber_refunds).values()), 0)
        self.assertEqual(settled.keeper_paid + settled.neutral_roll, 19)

    def test_join_requires_exact_current_share_not_a_future_subscriber(self) -> None:
        feed = SharedFeedReserve.empty(100)
        with self.assertRaises(ModelError):
            feed.join("first", 50)
        feed, _ = feed.join("first", 100)
        self.assertEqual(feed.required_join_deposit(), 50)


class FeeGeometryComparisonTests(unittest.TestCase):
    SCALE = 100
    # Midpoint-equivalent controls: flat 20 bp; dispersion and per-Egg
    # kappa 40 bp; quotient-norm kappa' 10 bp of range.
    FLAT = FeePolicy(FeeBasis.FLAT_CASH, 2, 1_000)
    DISPERSION = FeePolicy(FeeBasis.SIMPLEX_DISPERSION, 4, 1_000)
    PER_EGG = FeePolicy(FeeBasis.PER_EGG_LEG, 4, 1_000)
    QUOTIENT = FeePolicy(FeeBasis.QUOTIENT_RANGE, 1, 1_000)
    ALL_ARMS = (FLAT, DISPERSION, PER_EGG, QUOTIENT)

    def assert_same_ratio(self, left, right) -> None:  # type: ignore[no-untyped-def]
        self.assertEqual(
            left.base_numerator * right.base_denominator,
            right.base_numerator * left.base_denominator,
        )

    def test_midpoint_rates_are_exactly_calibrated(self) -> None:
        quotes = tuple(
            fee_quote((1_000, 0), (50, 50), self.SCALE, policy)
            for policy in self.ALL_ARMS
        )
        for left in quotes:
            for right in quotes:
                self.assertEqual(
                    left.exact_numerator * right.exact_denominator,
                    right.exact_numerator * left.exact_denominator,
                )

    def test_zero_and_certain_price_boundary_is_explicit(self) -> None:
        for price in (0, self.SCALE):
            prices = (price, self.SCALE - price)
            flat = fee_quote((100, 0), prices, self.SCALE, self.FLAT)
            dispersion = fee_quote(
                (100, 0), prices, self.SCALE, self.DISPERSION
            )
            if price == 0:
                self.assertEqual(flat.terminal_ceil_atoms, 0)
                self.assertEqual(dispersion.terminal_ceil_atoms, 0)
            else:
                self.assertGreater(flat.terminal_ceil_atoms, 0)
                self.assertEqual(dispersion.terminal_ceil_atoms, 0)

    def test_tail_relative_burden_crosses_at_midpoint(self) -> None:
        # Compare exact fee / exact cash base with cross multiplication.
        for price in range(1, self.SCALE + 1):
            flat = fee_quote((1_000, 0), (price, self.SCALE - price), self.SCALE, self.FLAT)
            dispersion = fee_quote(
                (1_000, 0),
                (price, self.SCALE - price),
                self.SCALE,
                self.DISPERSION,
            )
            flat_cash_num, flat_cash_den = flat_cash_base(
                (1_000, 0), (price, self.SCALE - price), self.SCALE
            )
            left = dispersion.exact_numerator * flat.exact_denominator
            right = flat.exact_numerator * dispersion.exact_denominator
            self.assertGreater(flat_cash_num, 0)
            self.assertGreater(flat_cash_den, 0)
            if price < self.SCALE // 2:
                self.assertGreater(left, right)
            elif price == self.SCALE // 2:
                self.assertEqual(left, right)
            else:
                self.assertLess(left, right)

    def test_complete_set_translation_only_dispersion_ignores_risk_free_cash(self) -> None:
        prices = (20, 30, 50)
        payoff = (0, 7, 20)
        translated = tuple(value + 11 for value in payoff)
        flat_before = flat_cash_base(payoff, prices, self.SCALE)
        flat_after = flat_cash_base(translated, prices, self.SCALE)
        disp_before = dispersion_base(payoff, prices, self.SCALE)
        disp_after = dispersion_base(translated, prices, self.SCALE)
        self.assertGreater(flat_after[0], flat_before[0])
        self.assertEqual(disp_after, disp_before)

    def test_complement_representation_is_symmetric_only_for_dispersion(self) -> None:
        prices = (10, 90)
        yes = (100, 0)
        no = (0, 100)
        self.assertNotEqual(
            flat_cash_base(yes, prices, self.SCALE),
            flat_cash_base(no, prices, self.SCALE),
        )
        self.assertEqual(
            dispersion_base(yes, prices, self.SCALE),
            dispersion_base(no, prices, self.SCALE),
        )

    def test_identical_payoff_partition_refinement_preserves_both_exact_bases(self) -> None:
        for prices in ((20, 30, 50), (0, 1, 99), (33, 33, 34)):
            payoffs = (0, 7, 20)
            for index, price in enumerate(prices):
                for left_price in range(price + 1):
                    refined_payoffs, refined_prices = split_identical_cell(
                        payoffs, prices, index, left_price
                    )
                    self.assertEqual(
                        flat_cash_base(payoffs, prices, self.SCALE),
                        flat_cash_base(
                            refined_payoffs, refined_prices, self.SCALE
                        ),
                    )
                    self.assertEqual(
                        dispersion_base(payoffs, prices, self.SCALE),
                        dispersion_base(
                            refined_payoffs, refined_prices, self.SCALE
                        ),
                    )

    def test_per_egg_leg_reduces_to_dispersion_on_every_single_egg(self) -> None:
        # Arm 3 is calibrated: on one Egg the benchmark and the candidate are
        # the same exact rational at every price composition.
        scale = 10
        for quantity in range(4):
            for first in range(scale + 1):
                for second in range(scale + 1 - first):
                    prices = (first, second, scale - first - second)
                    for index in range(3):
                        payoffs = tuple(
                            quantity if position == index else 0
                            for position in range(3)
                        )
                        self.assertEqual(
                            per_egg_leg_base(payoffs, prices, scale),
                            dispersion_base(payoffs, prices, scale),
                        )

    def test_dispersion_never_exceeds_per_egg_and_complete_sets_show_the_gap(
        self,
    ) -> None:
        # The benchmark the dispersion base was built to beat: charging leg by
        # leg ignores netting, so dispersion is never dearer and a risk-free
        # complete set displays the strict gap at interior prices.
        scale = 10
        for first in range(scale + 1):
            for second in range(scale + 1 - first):
                prices = (first, second, scale - first - second)
                for payoffs in itertools.product(range(4), repeat=3):
                    dispersion = dispersion_base(payoffs, prices, scale)
                    per_egg = per_egg_leg_base(payoffs, prices, scale)
                    self.assertEqual(dispersion[1], per_egg[1])
                    self.assertLessEqual(dispersion[0], per_egg[0])
        interior = (20, 30, 50)
        complete_set = (7, 7, 7)
        self.assertEqual(
            dispersion_base(complete_set, interior, self.SCALE)[0], 0
        )
        self.assertGreater(
            per_egg_leg_base(complete_set, interior, self.SCALE)[0], 0
        )

    def test_per_egg_is_refinement_sensitive_and_quotient_range_is_not(self) -> None:
        # Identical-payoff refinement preserves dispersion, flat cash, and the
        # range norm exactly; the per-Egg benchmark changes with the binning.
        payoffs = (0, 7, 20)
        prices = (2, 30, 68)
        refined_payoffs, refined_prices = split_identical_cell(
            payoffs, prices, 2, 1
        )
        self.assertEqual(
            quotient_range_base(payoffs, prices, self.SCALE),
            quotient_range_base(refined_payoffs, refined_prices, self.SCALE),
        )
        self.assertNotEqual(
            per_egg_leg_base(payoffs, prices, self.SCALE),
            per_egg_leg_base(refined_payoffs, refined_prices, self.SCALE),
        )

    def test_quotient_range_is_price_free_and_satisfies_every_seminorm_axiom(
        self,
    ) -> None:
        payoffs = (0, 7, 20)
        for prices in ((20, 30, 50), (0, 1, 99), (100, 0, 0), (33, 33, 34)):
            self.assertEqual(
                quotient_range_base(payoffs, prices, self.SCALE), (20, 1)
            )
        prices = (20, 30, 50)
        translated = tuple(value + 11 for value in payoffs)
        self.assertEqual(
            quotient_range_base(translated, prices, self.SCALE), (20, 1)
        )
        relabeled = (20, 0, 7)
        relabeled_prices = (50, 20, 30)
        self.assertEqual(
            quotient_range_base(relabeled, relabeled_prices, self.SCALE),
            quotient_range_base(payoffs, prices, self.SCALE),
        )
        tripled = tuple(value * 3 for value in payoffs)
        self.assertEqual(
            quotient_range_base(tripled, prices, self.SCALE), (60, 1)
        )

    def test_dispersion_is_bounded_by_a_quarter_of_the_range(self) -> None:
        # Proposition 10 (RISK_SUMMED_POSITIONS.md), bounded exhaustive:
        # 4 * G_num <= R(a) * S^2, with equality exactly at half mass on
        # argmax and half mass on argmin outcomes.
        scale = 10
        for first in range(scale + 1):
            for second in range(scale + 1 - first):
                prices = (first, second, scale - first - second)
                for payoffs in itertools.product(range(4), repeat=3):
                    numerator, _ = dispersion_base(payoffs, prices, scale)
                    range_norm = max(payoffs) - min(payoffs)
                    self.assertLessEqual(
                        4 * numerator, range_norm * scale * scale
                    )
        attained, _ = dispersion_base((4, 0, 2), (5, 5, 0), scale)
        self.assertEqual(4 * attained, 4 * scale * scale)

    def test_per_leg_terminal_rounding_is_a_partition_attack(self) -> None:
        payoffs = (100, 0)
        prices = (2, 98)
        whole = fee_quote(payoffs, prices, self.SCALE, self.DISPERSION)
        refined_payoffs, refined_prices = split_identical_cell(
            payoffs, prices, 0, 1
        )
        refined = fee_quote(
            refined_payoffs, refined_prices, self.SCALE, self.DISPERSION
        )
        self.assertEqual(whole.exact_numerator * refined.exact_denominator,
                         refined.exact_numerator * whole.exact_denominator)
        # Charging each artificial claim leg separately would add ceilings.
        leg_a = fee_quote((100, 0, 0), refined_prices, self.SCALE, self.DISPERSION)
        leg_b = fee_quote((0, 100, 0), refined_prices, self.SCALE, self.DISPERSION)
        self.assertGreater(
            leg_a.terminal_ceil_atoms + leg_b.terminal_ceil_atoms,
            refined.terminal_ceil_atoms,
        )

    def test_persistent_intent_carry_is_fragmentation_invariant(self) -> None:
        for policy in self.ALL_ARMS:
            for price in range(self.SCALE + 1):
                whole = fee_sequence(
                    ((31, 0),),
                    (price, self.SCALE - price),
                    self.SCALE,
                    policy,
                )
                for cut_a in range(32):
                    for cut_b in range(32 - cut_a):
                        cut_c = 31 - cut_a - cut_b
                        fragmented = fee_sequence(
                            ((cut_a, 0), (cut_b, 0), (cut_c, 0)),
                            (price, self.SCALE - price),
                            self.SCALE,
                            policy,
                        )
                        self.assertEqual(fragmented.total_paid, whole.total_paid)
                        self.assertEqual(
                            fragmented.exact_numerator, whole.exact_numerator
                        )

    def test_order_fee_headroom_covers_every_binary_price(self) -> None:
        for policy in self.ALL_ARMS:
            maximum = maximum_binary_single_egg_fee(1_001, self.SCALE, policy)
            for price in range(self.SCALE + 1):
                quote = fee_quote(
                    (1_001, 0),
                    (price, self.SCALE - price),
                    self.SCALE,
                    policy,
                )
                self.assertLessEqual(quote.terminal_ceil_atoms, maximum)

    def test_sybil_wash_never_recovers_treasury_and_zero_fee_pays_nobody(self) -> None:
        for basis in self.ALL_ARMS:
            for price in range(self.SCALE + 1):
                quote = fee_quote(
                    (10_000, 0),
                    (price, self.SCALE - price),
                    self.SCALE,
                    basis,
                )
                result = wash_result(quote, 60, 15, 100, 1_000, 5_000)
                self.assertLessEqual(result["collateral_net"], 0)
                if result["fee"] == 0:
                    self.assertEqual(result["recovered"], 0)
                else:
                    self.assertGreater(result["treasury"], 0)


class ZeroPriceLaunderingTests(unittest.TestCase):
    """Proposition 9 (RISK_SUMMED_POSITIONS.md) made executable.

    At boundary prices the dispersion kernel is every vector constant on the
    priced support, strictly larger than the risk quotient, so risk transfer
    supported entirely on zero-priced outcomes is feeless however large its
    model-free range.  This is the named zero-price laundering falsifier of
    FEE_GEOMETRY section 5.
    """

    SCALE = 100
    FLAT = FeeGeometryComparisonTests.FLAT
    DISPERSION = FeeGeometryComparisonTests.DISPERSION
    PER_EGG = FeeGeometryComparisonTests.PER_EGG
    QUOTIENT = FeeGeometryComparisonTests.QUOTIENT

    def test_boundary_kernel_is_exactly_constancy_on_the_priced_support(
        self,
    ) -> None:
        # Both directions of Proposition 9, bounded exhaustive: the dispersion
        # base vanishes if and only if the payoff is constant wherever the
        # price is positive, at boundary and interior prices alike.
        scale = 4
        for first in range(scale + 1):
            for second in range(scale + 1 - first):
                prices = (first, second, scale - first - second)
                for payoffs in itertools.product(range(4), repeat=3):
                    numerator, _ = dispersion_base(payoffs, prices, scale)
                    support = tuple(
                        payoff
                        for payoff, price in zip(payoffs, prices)
                        if price > 0
                    )
                    constant_on_support = len(set(support)) <= 1
                    self.assertEqual(numerator == 0, constant_on_support)

    def test_zero_price_supported_transfer_is_feeless_however_large(self) -> None:
        # The transfer varies only on zero-priced outcomes; its model-free
        # range is unbounded while every price-weighted arm charges exactly
        # zero -- flat cash and per-Egg share the hole because a zero-priced
        # leg has zero consideration.  Only the price-free quotient-norm arm
        # charges it, at exactly kappa' * R(a).
        prices = (0, 0, self.SCALE)
        for magnitude in (1, 10**6, 10**18, 10**30):
            payoffs = (magnitude, 0, 0)
            self.assertEqual(
                quotient_range_base(payoffs, prices, self.SCALE),
                (magnitude, 1),
            )
            for policy in (self.FLAT, self.DISPERSION, self.PER_EGG):
                quote = fee_quote(payoffs, prices, self.SCALE, policy)
                self.assertEqual(quote.base_numerator, 0)
                self.assertEqual(quote.exact_numerator, 0)
                self.assertEqual(quote.floor_atoms, 0)
                self.assertEqual(quote.terminal_ceil_atoms, 0)
                self.assertEqual(quote.carry, 0)
            quotient = fee_quote(payoffs, prices, self.SCALE, self.QUOTIENT)
            self.assertEqual(quotient.floor_atoms, magnitude // 1_000)
            self.assertEqual(
                quotient.terminal_ceil_atoms, ceil_div(magnitude, 1_000)
            )

    def test_terminal_ceil_cannot_rescue_the_dispersion_hole(self) -> None:
        # Fragmenting the transfer and closing the intent still pays zero
        # under dispersion: the carry never becomes nonzero, so the terminal
        # ceiling has nothing to round up.
        prices = (0, 0, self.SCALE)
        magnitude = 10**30
        fragments = ((magnitude, 0, 0),) * 3
        sequence = fee_sequence(fragments, prices, self.SCALE, self.DISPERSION)
        self.assertEqual(sequence.total_paid, 0)
        self.assertEqual(sequence.final_carry, 0)
        self.assertEqual(sequence.exact_numerator, 0)
        quotient_sequence = fee_sequence(
            fragments, prices, self.SCALE, self.QUOTIENT
        )
        self.assertEqual(quotient_sequence.total_paid, 3 * magnitude // 1_000)


if __name__ == "__main__":
    unittest.main()
