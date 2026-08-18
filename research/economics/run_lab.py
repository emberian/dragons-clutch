#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Run deterministic Dragon's Clutch cryptoeconomic scenarios."""

from __future__ import annotations

import json
from fractions import Fraction

from model import (
    ALLOWED_POOL_PURPOSES,
    Pool,
    ProtectedPools,
    ReverseDutchSchedule,
    SharedFeedEpoch,
    allocate_fee,
    common_mode_exposure,
    dominant_tail_attack,
    enumerate_solvency_traces,
    exhaustive_liveness_orders,
    exposure_admissible,
    fee_fragmentation_result,
    fee_with_carry,
    integer_shares,
    midpoint_effective_bps,
    required_weighted_volume,
    single_egg_dispersion_numerator,
)


PRICE_SCALE = 10_000
QUANTITY = 1_000_000
KAPPA_NUM = 4
KAPPA_DEN = 1_000


def fraction_text(value: Fraction) -> str:
    if value.denominator == 1:
        return str(value.numerator)
    return f"{value.numerator}/{value.denominator}"


def pool_refusal_count() -> int:
    balances = ProtectedPools(10, 10, 10, 10, 10, 10, 10)
    purposes = sorted({purpose for values in ALLOWED_POOL_PURPOSES.values() for purpose in values})
    refused = 0
    for pool in Pool:
        for purpose in purposes:
            if purpose in ALLOWED_POOL_PURPOSES[pool]:
                balances.debit(pool, purpose, 1)
            else:
                try:
                    balances.debit(pool, purpose, 1)
                except ValueError:
                    refused += 1
                else:
                    raise AssertionError("forbidden protected-pool debit succeeded")
    return refused


def shared_feed_scenario() -> dict[str, object]:
    epoch = SharedFeedEpoch.first(101)
    joins: list[dict[str, object]] = [
        {"subscriber_count": 1, "deposit": 101, "capital_shares": [101]}
    ]
    while len(epoch.capital_shares) < 8:
        epoch, result = epoch.join()
        joins.append(
            {
                "subscriber_count": result.subscriber_count,
                "deposit": result.deposit,
                "reimbursements": list(result.reimbursements),
                "capital_shares": list(result.capital_shares),
            }
        )
    success = epoch.settle(37, success=True)
    failure = epoch.settle(37, success=False)
    return {
        "reserve_cap_sol_atoms": epoch.reserve_cap,
        "joins": joins,
        "success": {
            "keeper_paid": success.keeper_paid,
            "subscriber_costs": list(success.subscriber_costs),
            "subscriber_refunds": list(success.subscriber_refunds),
            "neutral_reserve_roll": success.neutral_reserve_roll,
        },
        "failure": {
            "keeper_paid": failure.keeper_paid,
            "subscriber_costs": list(failure.subscriber_costs),
            "subscriber_refunds": list(failure.subscriber_refunds),
            "neutral_reserve_roll": failure.neutral_reserve_roll,
            "note": "failure returns nothing to subscribers; residual rolls to a neutral source reserve",
        },
    }


def fee_curve() -> list[dict[str, object]]:
    rows = []
    for price in (500, 1_000, 2_500, 5_000, 7_500, 9_000, 9_500):
        base = single_egg_dispersion_numerator(QUANTITY, price, PRICE_SCALE)
        fee, carry = fee_with_carry(base, PRICE_SCALE, KAPPA_NUM, KAPPA_DEN)
        consideration = QUANTITY * price // PRICE_SCALE
        effective_bps = Fraction(fee * 10_000, consideration) if consideration else Fraction(0)
        allocation = allocate_fee(fee)
        rows.append(
            {
                "price": fraction_text(Fraction(price, PRICE_SCALE)),
                "fee_collateral_atoms": fee,
                "carry": carry,
                "cash_consideration_atoms": consideration,
                "effective_bps": fraction_text(effective_bps),
                "maker_atoms": allocation.maker,
                "executor_atoms": allocation.executor,
                "treasury_atoms": allocation.treasury,
            }
        )
    return rows


def break_even_table() -> list[dict[str, object]]:
    rows = []
    operating_cost = Fraction(100)
    service_premium = Fraction(10)
    treasury_share = Fraction(1, 4)
    for kappa in (
        Fraction(0),
        Fraction(1, 1_000),
        Fraction(2, 1_000),
        Fraction(4, 1_000),
        Fraction(7, 1_000),
        Fraction(1, 100),
    ):
        for sol_per_collateral in (
            Fraction(1, 100_000),
            Fraction(1, 1_000_000),
            Fraction(1, 10_000_000),
            Fraction(0),
        ):
            required = required_weighted_volume(
                operating_cost,
                service_premium,
                kappa,
                treasury_share,
                sol_per_collateral,
            )
            rows.append(
                {
                    "kappa": fraction_text(kappa),
                    "midpoint_gross_bps": fraction_text(midpoint_effective_bps(kappa)),
                    "sol_per_collateral_atom": fraction_text(sol_per_collateral),
                    "required_weighted_volume_atoms": (
                        "unbounded" if required is None else fraction_text(required)
                    ),
                    "required_midpoint_cash_volume_atoms": (
                        "unbounded" if required is None else fraction_text(2 * required)
                    ),
                }
            )
    return rows


def price_collapse_table() -> list[dict[str, object]]:
    hoard = 1_000_000
    liability = 900_000
    fee_revenue = 50_000
    keeper_reward = 10_000
    prepaid_sol = 500_000
    rows = []
    for sol_per_dregg in (
        Fraction(1, 100_000),
        Fraction(1, 1_000_000),
        Fraction(1, 10_000_000),
        Fraction(0),
    ):
        rows.append(
            {
                "sol_per_dregg_atom": fraction_text(sol_per_dregg),
                "hoard_atoms": hoard,
                "liability_atoms": liability,
                "atom_margin": hoard - liability,
                "hoard_value_sol": fraction_text(hoard * sol_per_dregg),
                "liability_value_sol": fraction_text(liability * sol_per_dregg),
                "fee_revenue_value_sol": fraction_text(fee_revenue * sol_per_dregg),
                "keeper_reward_value_sol": fraction_text(keeper_reward * sol_per_dregg),
                "prepaid_liveness_sol_atoms": prepaid_sol,
            }
        )
    return rows


def main() -> None:
    attack_rows = []
    for outcomes in (2, 4, 16):
        attack = dominant_tail_attack(outcomes, 1_000_000, Fraction(1, 100))
        attack_rows.append(
            {
                "outcomes": outcomes,
                **{key: fraction_text(value) for key, value in attack.items()},
            }
        )

    exposure = common_mode_exposure(
        (1_000_000, 500_000, 250_000),
        (Fraction(1), Fraction(1, 2), Fraction(1, 4)),
    )
    schedule = ReverseDutchSchedule((12, 18, 27, 40))
    report = {
        "schema": "dragons-clutch-economics-lab-v1",
        "status": "synthetic deterministic hypotheses; not protocol constants",
        "arithmetic": "integer and exact rational only",
        "scenario": {
            "generator": "bounded exhaustive checks plus fixed scenario tables",
            "random_seed": "none",
            "fixtures": "synthetic only",
            "source_revision": "working tree; bind revision before treating output as evidence",
        },
        "solvency": enumerate_solvency_traces(),
        "protected_pools": {"forbidden_debits_refused": pool_refusal_count()},
        "liveness": {
            "booking_order_checks": exhaustive_liveness_orders(),
            "reverse_dutch_offers_sol_atoms": list(schedule.offers),
            "booked_maximum_sol_atoms": schedule.booked_maximum,
            "future_revenue_counted": False,
        },
        "shared_feed": shared_feed_scenario(),
        "failure_attack": {
            "equal_failure_tail_basket": attack_rows,
            "common_mode_exposure_atoms": fraction_text(exposure),
            "admissible_at_ten_percent_of_20m_cost_bound": exposure_admissible(
                exposure, Fraction(20_000_000)
            ),
        },
        "fee_hypothesis": {
            "kappa": "1/250",
            "midpoint_gross_bps": fraction_text(midpoint_effective_bps(Fraction(1, 250))),
            "allocation_hypothesis": {"maker_percent": 60, "executor_percent_max": 15, "treasury_percent_min": 25},
            "curve": fee_curve(),
            "fragmentation_1001_single_atom_fills": fee_fragmentation_result(
                (1,) * 1_001, 5_000, PRICE_SCALE, KAPPA_NUM, KAPPA_DEN
            ),
        },
        "price_collapse": price_collapse_table(),
        "break_even_sensitivity": {
            "operating_cost_sol": "100",
            "service_premium_sol": "10",
            "treasury_share": "1/4",
            "rows": break_even_table(),
        },
    }
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
