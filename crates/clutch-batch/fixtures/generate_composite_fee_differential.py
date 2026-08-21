#!/usr/bin/env python3
"""Emit the composite fee-base lab differential consumed by clutch-batch.

Every expected value in ``composite_fee_lab_differential.txt`` is produced by
``research/economics-admission/model.py``'s ``composite_floor_quote`` — the
unbounded-integer Python model the fee-base selection report ran — never by the
Rust under test.  ``relation_v1::composite_fee_quote`` must reproduce each row
exactly, or refuse for the stated reason.

Both rates ride ``FEE_BPS_DENOMINATOR = 10_000`` because that is the only rate
denominator the Rust ``FeeBaseV1`` carries; the lab's ``FeePolicy`` is
instantiated with that denominator so the two sides quote the same rational.

Run from the repository root:

    python3 crates/clutch-batch/fixtures/generate_composite_fee_differential.py \
        > crates/clutch-batch/fixtures/composite_fee_lab_differential.txt
"""

from __future__ import annotations

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "research" / "economics-admission"))

from model import FeeBasis, FeePolicy, composite_floor_quote  # noqa: E402

# Mirrors of the Rust bounds this differential is written against.
FEE_BPS_DENOMINATOR = 10_000
PRICE_SCALE = 10_000
MAX_OUTCOMES = 16
U64_MAX = 2**64 - 1
U128_MAX = 2**128 - 1
MAX_COMPOSITE_PRICE_SCALE = 10**15

# The lab's comparison calibration (report section 3.1): kappa = 40 bp of the
# dispersion base, kappa' = 10 bp of the model-free range.  A calibration, never
# a proposed production rate.
LAB_KAPPA = 40
LAB_FLOOR = 10


def row(name, payoffs, prices, price_scale, dispersion_bps, floor_range_bps, prior_carry=0):
    return {
        "name": name,
        "payoffs": list(payoffs),
        "prices": list(prices),
        "price_scale": price_scale,
        "dispersion_bps": dispersion_bps,
        "floor_range_bps": floor_range_bps,
        "prior_carry": prior_carry,
    }


def rows():
    out = []

    # --- the report's section 3.1 measured grid, composite column -----------
    # One 10,000-atom binary claim at price scale S = 100, at the lab
    # calibration.  These seven rows reproduce the published table.
    for price in (0, 1, 10, 50, 90, 99, 100):
        out.append(
            row(
                f"report_grid_price_{price}",
                [10_000, 0],
                [price, 100 - price],
                100,
                LAB_KAPPA,
                LAB_FLOOR,
            )
        )

    # --- the zero-rate regression anchor -----------------------------------
    # Both rates zero must quote exactly zero on every shape, including the
    # boundary-price channel: this is the byte-for-byte "nothing changed" row.
    for name, payoffs, prices, scale in (
        ("zero_rates_midpoint", [10_000, 0], [5_000, 5_000], PRICE_SCALE),
        ("zero_rates_zero_price", [10**18, 0, 0], [0, 0, PRICE_SCALE], PRICE_SCALE),
        ("zero_rates_complete_set", [7, 7, 7], [3_000, 3_000, 4_000], PRICE_SCALE),
    ):
        out.append(row(name, payoffs, prices, scale, 0, 0))

    # --- the zero-price laundering channel, at u64-representable scale ------
    # Prices (0, 0, S): dispersion's kernel swallows the whole transfer
    # (G_num = 0), so the entire charge is the price-free floor.  This is the
    # section 3.2 falsifier, scaled into the relation's u64 payoff domain.
    for magnitude in (10**6, 10**12, 10**18, U64_MAX):
        out.append(
            row(
                f"zero_price_channel_{magnitude}",
                [magnitude, 0, 0],
                [0, 0, PRICE_SCALE],
                PRICE_SCALE,
                LAB_KAPPA,
                LAB_FLOOR,
            )
        )
    # The same channel at the lab's own S = 100 and at a one-tick price, where
    # bare dispersion charges essentially nothing and the floor charges kappa'R.
    out.append(row("zero_price_channel_lab_scale", [10**18, 0, 0], [0, 0, 100], 100, LAB_KAPPA, LAB_FLOOR))
    out.append(row("one_tick_price", [10**8, 0], [1, PRICE_SCALE - 1], PRICE_SCALE, LAB_KAPPA, LAB_FLOOR))

    # --- the kernel: complete sets are free at every price ------------------
    for name, payoffs, prices in (
        ("kernel_complete_set_interior", [9, 9, 9], [2_500, 2_500, 5_000]),
        ("kernel_complete_set_boundary", [9, 9, 9], [0, 0, PRICE_SCALE]),
        ("kernel_zero_vector", [0, 0], [5_000, 5_000]),
    ):
        out.append(row(name, payoffs, prices, PRICE_SCALE, LAB_KAPPA, LAB_FLOOR))

    # --- the wash fixture: an offsetting round trip charges twice -----------
    # Each leg of a two-owner wash is quoted on its own filled vector; the pair
    # is strictly costly because neither leg is in the kernel.
    out.append(row("wash_leg_a", [4, 0], [5_000, 5_000], PRICE_SCALE, LAB_KAPPA, LAB_FLOOR))
    out.append(row("wash_leg_b", [0, 4], [5_000, 5_000], PRICE_SCALE, LAB_KAPPA, LAB_FLOOR))
    out.append(row("wash_netted_complete_set", [4, 4], [5_000, 5_000], PRICE_SCALE, LAB_KAPPA, LAB_FLOOR))

    # --- boundary rates -----------------------------------------------------
    for name, dispersion, floor in (
        ("rate_boundary_both_max", FEE_BPS_DENOMINATOR, FEE_BPS_DENOMINATOR),
        ("rate_dispersion_only_max", FEE_BPS_DENOMINATOR, 0),
        ("rate_floor_only_max", 0, FEE_BPS_DENOMINATOR),
        ("rate_smallest_nonzero", 1, 1),
        ("rate_dispersion_only_one", 1, 0),
        ("rate_floor_only_one", 0, 1),
    ):
        out.append(row(name, [10_000, 3, 0], [4_000, 1_000, 5_000], PRICE_SCALE, dispersion, floor))

    # --- dust: the sub-atom regime the carry exists for ---------------------
    for quantity in (1, 2, 3):
        out.append(
            row(
                f"dust_fill_{quantity}",
                [quantity, 0],
                [5_000, 5_000],
                PRICE_SCALE,
                LAB_KAPPA,
                LAB_FLOOR,
            )
        )

    # --- a nonzero prior carry: the fragmentation-invariance input ----------
    denominator = FEE_BPS_DENOMINATOR * PRICE_SCALE * PRICE_SCALE * FEE_BPS_DENOMINATOR
    for carry in (1, denominator // 3, denominator - 1):
        out.append(
            row(
                f"prior_carry_{carry}",
                [5, 0],
                [5_000, 5_000],
                PRICE_SCALE,
                LAB_KAPPA,
                LAB_FLOOR,
                prior_carry=carry,
            )
        )

    # --- wider outcome vectors ---------------------------------------------
    out.append(
        row(
            "four_outcome_portfolio",
            [11, 4, 0, 7],
            [1_000, 2_500, 500, 6_000],
            PRICE_SCALE,
            LAB_KAPPA,
            LAB_FLOOR,
        )
    )
    wide_payoffs = [i * 1_000 for i in range(MAX_OUTCOMES)]
    wide_prices = [625] * MAX_OUTCOMES
    out.append(
        row("sixteen_outcome_ramp", wide_payoffs, wide_prices, PRICE_SCALE, LAB_KAPPA, LAB_FLOOR)
    )
    ragged_prices = [0] * MAX_OUTCOMES
    ragged_prices[0] = PRICE_SCALE - 15
    for i in range(1, MAX_OUTCOMES):
        ragged_prices[i] = 1
    out.append(
        row("sixteen_outcome_near_boundary", wide_payoffs, ragged_prices, PRICE_SCALE, LAB_KAPPA, LAB_FLOOR)
    )

    # --- the widest payoff the relation can represent, at its real scale ----
    out.append(
        row(
            "u64_max_payoff_at_price_scale",
            [U64_MAX, 0],
            [5_000, 5_000],
            PRICE_SCALE,
            FEE_BPS_DENOMINATOR,
            FEE_BPS_DENOMINATOR,
        )
    )

    # --- the checked-width boundary ----------------------------------------
    # A price scale the denominator still admits but whose numerator does not
    # fit u128: the relation must refuse, never wrap.
    out.append(
        row(
            "numerator_overflows_u128",
            [U64_MAX, 0],
            [MAX_COMPOSITE_PRICE_SCALE // 2, MAX_COMPOSITE_PRICE_SCALE - MAX_COMPOSITE_PRICE_SCALE // 2],
            MAX_COMPOSITE_PRICE_SCALE,
            LAB_KAPPA,
            LAB_FLOOR,
        )
    )
    # One price scale past the denominator bound entirely.
    out.append(
        row(
            "price_scale_past_composite_bound",
            [4, 0],
            [MAX_COMPOSITE_PRICE_SCALE // 2 + 1, MAX_COMPOSITE_PRICE_SCALE // 2],
            MAX_COMPOSITE_PRICE_SCALE + 1,
            LAB_KAPPA,
            LAB_FLOOR,
        )
    )

    # --- the lab's own laundering falsifier, verbatim ----------------------
    # Payoffs (10^30, 0, 0) at prices (0, 0, 100): the row the selection report
    # published as 10^27 atoms charged.  10^30 is not a u64, so the relation
    # cannot represent the vector at all; the row is carried so the differential
    # states that bound rather than omitting it.
    out.append(row("lab_falsifier_10e30", [10**30, 0, 0], [0, 0, 100], 100, LAB_KAPPA, LAB_FLOOR))

    return out


def classify(record, quote):
    """What the Rust must do with this row, and why."""

    if any(value > U64_MAX for value in record["payoffs"]):
        return "payoff_not_u64"
    if record["price_scale"] > MAX_COMPOSITE_PRICE_SCALE:
        return "price_scale_out_of_domain"
    if quote.exact_denominator > U128_MAX or quote.exact_numerator > U128_MAX:
        return "overflow"
    return "ok"


def main():
    print("# Composite fee-base lab differential.")
    print("#")
    print("# GENERATED — do not hand-edit.  Regenerate with")
    print("#   python3 crates/clutch-batch/fixtures/generate_composite_fee_differential.py \\")
    print("#     > crates/clutch-batch/fixtures/composite_fee_lab_differential.txt")
    print("#")
    print("# Every expected value comes from research/economics-admission/model.py's")
    print("# composite_floor_quote at rate denominator 10_000, the only denominator")
    print("# FeeBaseV1 carries.  relation_v1::composite_fee_quote must agree on every")
    print("# `expect ok` row and refuse for the stated reason on every other one.")
    print("#")
    print("# expect ok                        -> quote equals the listed fields exactly")
    print("# expect overflow                  -> Err(ArithmeticOverflow): the exact")
    print("#                                     rational does not fit u128")
    print("# expect price_scale_out_of_domain -> Err(InvalidPriceScale)")
    print("# expect payoff_not_u64            -> the vector is outside the relation's")
    print("#                                     payoff domain and is not quotable")
    for record in rows():
        dispersion_policy = FeePolicy(
            basis=FeeBasis.SIMPLEX_DISPERSION,
            rate_numerator=record["dispersion_bps"],
            rate_denominator=FEE_BPS_DENOMINATOR,
        )
        floor_policy = FeePolicy(
            basis=FeeBasis.QUOTIENT_RANGE,
            rate_numerator=record["floor_range_bps"],
            rate_denominator=FEE_BPS_DENOMINATOR,
        )
        quote = composite_floor_quote(
            record["payoffs"],
            record["prices"],
            record["price_scale"],
            dispersion_policy,
            floor_policy,
            record["prior_carry"],
        )
        expect = classify(record, quote)
        print()
        print(f"row {record['name']}")
        print("payoffs " + " ".join(str(v) for v in record["payoffs"]))
        print("prices " + " ".join(str(v) for v in record["prices"]))
        print(f"price_scale {record['price_scale']}")
        print(f"dispersion_bps {record['dispersion_bps']}")
        print(f"floor_range_bps {record['floor_range_bps']}")
        print(f"prior_carry {record['prior_carry']}")
        print(f"expect {expect}")
        if expect == "ok":
            print(f"base_numerator {quote.base_numerator}")
            print(f"base_denominator {quote.base_denominator}")
            print(f"exact_numerator {quote.exact_numerator}")
            print(f"exact_denominator {quote.exact_denominator}")
            print(f"floor_atoms {quote.floor_atoms}")
            print(f"terminal_ceil_atoms {quote.terminal_ceil_atoms}")
            print(f"carry {quote.carry}")


if __name__ == "__main__":
    main()
