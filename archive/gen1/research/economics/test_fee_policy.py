# SPDX-License-Identifier: AGPL-3.0-or-later
"""Fee-policy falsifiers: payer debit, carry domains, dispersion base, wash.

Covers POLICY_ANALYSIS_LOTS_FEES.md sections 2.2-2.5 and 3.3 and the EXP-FEE
rows of its section 5 matrix.  ``kappa = 4/1000`` and the 60/15/25 allocation
appear here only as experimental arms.
"""

from __future__ import annotations

import unittest
from fractions import Fraction

from experiments import (
    exp_fee_a1,
    exp_fee_d1,
    exp_fee_d2,
    exp_fee_g1,
    exp_fee_g2,
    exp_fee_p1,
    exp_fee_p2,
    exp_fee_w1,
)
from model import (
    CarryAccount,
    CarryClose,
    CarryDomain,
    FeeSideArm,
    Fill,
    ModelError,
    allocate_fee,
    dispersion_numerator,
    escrow_reservation,
    exact_consideration,
    fee_denominator,
    fee_fragmentation_result,
    fee_numerator,
    max_single_egg_fee_numerator,
    run_fee_schedule,
    single_egg_dispersion_numerator,
    sybil_wash_result,
)

SCALE = 100
KAPPA_NUM = 4
KAPPA_DEN = 1_000


class PayerDebitAccounting(unittest.TestCase):
    """Section 2.3: fee legs debit a named payer, never the Hoard."""

    def test_single_fill_legs_have_the_documented_shape(self) -> None:
        # PROPOSED variant, explicitly named (P0-5)
        result = run_fee_schedule(
            [Fill(2000, 50)],
            SCALE,
            KAPPA_NUM,
            KAPPA_DEN,
            domain=CarryDomain.INTENT,
            close_policy=CarryClose.TERMINAL_CEIL,
            side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
        )
        consideration = exact_consideration(2000, 50, SCALE)
        self.assertEqual(consideration, 1_000)
        self.assertEqual(result.buyer_debit_total, consideration + 2)
        self.assertEqual(result.seller_credit_total, consideration - 2)
        self.assertEqual(result.fee_pot, 4)
        self.assertTrue(result.conserves)
        self.assertEqual(result.hoard_delta, 0)

    def test_off_grid_consideration_is_refused_rather_than_floored(self) -> None:
        with self.assertRaises(ModelError):
            exact_consideration(3, 50, SCALE)

    def test_escrow_reserves_worst_case_fee_head_room(self) -> None:
        """A buyer intent with limit price 50 reserves limit cash plus fee head-room."""

        denominator = fee_denominator(KAPPA_DEN, SCALE)
        limit_price = 50
        worst = max_single_egg_fee_numerator(2000, SCALE, KAPPA_NUM)
        limit_consideration = exact_consideration(2000, limit_price, SCALE)
        reservation = escrow_reservation(limit_consideration, worst, denominator)
        self.assertEqual(worst, KAPPA_NUM * 2000 * 50 * 50)
        self.assertGreater(reservation, limit_consideration)
        for price in range(0, limit_price + 1):
            if (2000 * price) % SCALE:
                continue
            # PROPOSED variant, explicitly named (P0-5)
            result = run_fee_schedule(
                [Fill(2000, price)],
                SCALE,
                KAPPA_NUM,
                KAPPA_DEN,
                domain=CarryDomain.INTENT,
                close_policy=CarryClose.TERMINAL_CEIL,
                side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
            )
            self.assertLessEqual(dict(result.intent_cash)["buy-1"], reservation)

    def test_exp_fee_p1_payer_conservation(self) -> None:
        result = exp_fee_p1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["schedules"], 500)

    def test_exp_fee_p2_side_arms_are_different_policies(self) -> None:
        result = exp_fee_p2()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["dust_points"], 20)
        # Below the one-atom terminal-ceil floor the arms are indistinguishable.
        self.assertTrue(
            all(
                row["charge_once_pot"] == row["both_sides_pot"]
                for row in result.data["dust_rows"]
            )
        )
        # Above it, charge-once-split collects about half the venue take.
        differing = [
            row
            for row in result.data["supra_atom_rows"]
            if row["charge_once_pot"] != row["both_sides_pot"]
        ]
        self.assertTrue(differing)
        for row in differing:
            self.assertLessEqual(row["charge_once_pot"] * 2, row["both_sides_pot"] + 2)


class CarryDomainPolicy(unittest.TestCase):
    """Section 2.2: terminal-ceil carry versus naive dropped carry."""

    def test_carry_account_charges_exactly_the_ceiling_at_close(self) -> None:
        account = CarryAccount(denominator=7)
        for numerator in (3, 3, 3):
            account, _ = account.charge(numerator)
        self.assertEqual(account.paid, 1)
        self.assertEqual(account.carry, 2)
        closed, extra = account.close(CarryClose.TERMINAL_CEIL)
        self.assertEqual(extra, 1)
        self.assertEqual(closed.paid, 2)
        self.assertEqual(closed.paid, account.exact_ceiling)
        dropped, extra = account.close(CarryClose.DROPPED)
        self.assertEqual(extra, 0)
        self.assertEqual(dropped.paid, 1)

    def test_fragmentation_arms_are_reported_side_by_side(self) -> None:
        result = fee_fragmentation_result((1,) * 8, 50, SCALE, KAPPA_NUM, KAPPA_DEN)
        self.assertEqual(result["dropped_carry_total"], result["persistent_total"])
        self.assertEqual(
            result["terminal_ceil_total"],
            result["persistent_total"] + (1 if result["persistent_carry"] else 0),
        )
        self.assertEqual(result["terminal_ceil_total"], result["exact_ceil_total"])
        self.assertLessEqual(result["reset_total"], result["terminal_ceil_total"])

    def test_all_three_domains_are_reachable_and_keyed_differently(self) -> None:
        fills = [
            Fill(2, 50, buyer_intent="buy-a", seller_intent="sell-a", epoch=0),
            Fill(2, 50, buyer_intent="buy-b", seller_intent="sell-b", epoch=1),
        ]
        pots = {}
        for domain in CarryDomain:
            result = run_fee_schedule(
                fills,
                SCALE,
                KAPPA_NUM,
                KAPPA_DEN,
                domain=domain,
                close_policy=CarryClose.TERMINAL_CEIL,
                # PROPOSED variant, explicitly named (P0-5)
                side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
            )
            pots[domain.value] = result.fee_pot
        self.assertEqual(pots["intent"], 4)
        self.assertEqual(pots["position"], 2)
        self.assertEqual(pots["epoch"], 4)

    def test_exp_fee_d1_terminal_ceil_is_fragmentation_exact(self) -> None:
        result = exp_fee_d1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["compositions"], 5_000)
        self.assertGreater(result.counts["cross_domain_schedules"], 100)
        self.assertEqual(result.counts["max_reset_gain"], 0)

    def test_exp_fee_d2_epoch_dropped_carry_loses_dust_fees(self) -> None:
        result = exp_fee_d2()
        self.assertFalse(result.falsified, msg=result.witnesses)
        rows = result.data["rows"]
        self.assertTrue(all(row["epoch_dropped_pot"] == 0 for row in rows))
        self.assertTrue(all(row["volume"] > 0 for row in rows))
        self.assertTrue(all(row["intent_terminal_ceil_pot"] > 0 for row in rows))


class DispersionBase(unittest.TestCase):
    """Section 2.4: the exact-integer dispersion fee base."""

    def test_single_egg_reduction_matches_the_documented_curve(self) -> None:
        for price in range(0, SCALE + 1):
            payoffs = (3, 0)
            prices = (price, SCALE - price)
            self.assertEqual(
                dispersion_numerator(payoffs, prices),
                single_egg_dispersion_numerator(3, price, SCALE),
            )

    def test_flat_notional_fee_is_not_on_the_dispersion_curve(self) -> None:
        """Section 2.4: `matched * bps / 10^4` is price-independent, so no kappa fits."""

        quantity = 1_000
        flat = {
            price: quantity * 30 // 10_000 for price in (2_500, 5_000, 7_500)
        }
        self.assertEqual(len(set(flat.values())), 1)
        curve = {
            price: single_egg_dispersion_numerator(quantity, price, 10_000)
            for price in (2_500, 5_000, 7_500)
        }
        self.assertNotEqual(curve[2_500], curve[5_000])

    def test_exp_fee_g1_seminorm_identities(self) -> None:
        result = exp_fee_g1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        for key, value in result.counts.items():
            self.assertGreater(value, 1_000, msg=key)

    def test_exp_fee_g2_width_proposal_fits_u128(self) -> None:
        result = exp_fee_g2()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["verified_cells"], 50)
        for row in result.data["rows"]:
            self.assertTrue(row["fits_u128"])
            self.assertGreater(row["u128_headroom_bits"], 0)

    def test_fee_numerator_and_denominator_are_the_documented_products(self) -> None:
        dispersion = dispersion_numerator((3, 0, 1), (2, 3, 5))
        self.assertEqual(dispersion, 53)
        self.assertEqual(fee_numerator(2, dispersion, KAPPA_NUM), 2 * 4 * 53)
        self.assertEqual(fee_denominator(KAPPA_DEN, 10), 1_000 * 100)


class AllocationAndWash(unittest.TestCase):
    """Sections 2.3 and 2.5: allocation with an executor cap, and self-wash sign."""

    def test_executor_cap_moves_atoms_to_treasury_without_losing_any(self) -> None:
        # PROPOSED variant, explicitly named (P0-5)
        uncapped = allocate_fee(
            1_000, maker_num=60, executor_num=15, denominator=100, executor_cap=None
        )
        capped = allocate_fee(
            1_000, maker_num=60, executor_num=15, denominator=100, executor_cap=100
        )
        self.assertEqual(uncapped.total, 1_000)
        self.assertEqual(capped.total, 1_000)
        self.assertEqual(capped.executor, 100)
        self.assertEqual(capped.treasury, uncapped.treasury + 50)

    def test_wash_is_strictly_negative_under_terminal_ceil(self) -> None:
        fills = [Fill(2, 50, buyer_intent=f"buy-{i}", seller_intent=f"sell-{i}") for i in range(4)]
        for side_arm in FeeSideArm:
            # PROPOSED variant, explicitly named (P0-5)
            result = sybil_wash_result(
                fills,
                SCALE,
                KAPPA_NUM,
                KAPPA_DEN,
                domain=CarryDomain.INTENT,
                close_policy=CarryClose.TERMINAL_CEIL,
                side_arm=side_arm,
                maker_num=60,
                executor_num=15,
                denominator=100,
                executor_cap=None,
            )
            self.assertLess(result["net_wash"], 0)
            self.assertLessEqual(result["recovered"] * 100, result["fee_pot"] * 75)

    def test_exp_fee_w1_no_configuration_pays_a_washer(self) -> None:
        result = exp_fee_w1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertEqual(result.counts["cells"], 36)
        self.assertGreater(result.counts["zero_fee_cells"], 0)

    def test_exp_fee_a1_allocation_is_exact_under_every_cap(self) -> None:
        result = exp_fee_a1()
        self.assertFalse(result.falsified, msg=result.witnesses)
        self.assertGreater(result.counts["allocations"], 10_000)

    def test_midpoint_effective_burden_is_still_twenty_basis_points(self) -> None:
        """The section 2.3 arithmetic behind the 60/15/25 wash argument, restated."""

        kappa = Fraction(KAPPA_NUM, KAPPA_DEN)
        price = Fraction(1, 2)
        gross = kappa * price * (1 - price)
        consideration = price
        self.assertEqual(gross / consideration * 10_000, 20)


if __name__ == "__main__":
    unittest.main()
