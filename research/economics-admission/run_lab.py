# SPDX-License-Identifier: AGPL-3.0-or-later
"""Emit a stable JSON summary of the admission and fee-policy contrasts."""

from __future__ import annotations

import json

from model import (
    AdmissionFunding,
    FeeBasis,
    FeePolicy,
    MandatoryJob,
    SharedFeedReserve,
    admit_market,
    fee_quote,
    quote_admission,
)


def ratio(numerator: int, denominator: int) -> str:
    return f"{numerator}/{denominator}"


def main() -> None:
    jobs = (
        MandatoryJob("observe", 7, 2, 3),
        MandatoryJob("repair", 11, 5, 4),
        MandatoryJob("finalize", 13, 1, 0),
    )
    feed = SharedFeedReserve.empty(17)
    admission = quote_admission(jobs, feed)
    admitted = admit_market(
        "example-market",
        jobs,
        feed,
        AdmissionFunding(31, 8, "EXAMPLE-REWARD", 7, 17),
    )
    policies = (
        FeePolicy(FeeBasis.FLAT_CASH, 2, 1_000),
        FeePolicy(FeeBasis.SIMPLEX_DISPERSION, 4, 1_000),
        FeePolicy(FeeBasis.PER_EGG_LEG, 4, 1_000),
        FeePolicy(FeeBasis.QUOTIENT_RANGE, 1, 1_000),
    )
    fee_rows = []
    for price in (0, 1, 10, 50, 90, 99, 100):
        for policy in policies:
            quote = fee_quote((10_000, 0), (price, 100 - price), 100, policy)
            fee_rows.append(
                {
                    "basis": policy.basis.value,
                    "base": ratio(quote.base_numerator, quote.base_denominator),
                    "exact_fee": ratio(
                        quote.exact_numerator, quote.exact_denominator
                    ),
                    "price": price,
                    "terminal_ceil_atoms": quote.terminal_ceil_atoms,
                }
            )
    # Proposition 9 falsifier row: risk transfer supported entirely on
    # zero-priced outcomes.  Every price-weighted arm charges it zero however
    # large its model-free range; only the quotient-norm arm charges it.
    laundering_payoffs = (10**30, 0, 0)
    laundering_prices = (0, 0, 100)
    zero_price_laundering = {
        "payoffs": list(laundering_payoffs),
        "prices": list(laundering_prices),
        "model_free_range": 10**30,
        "terminal_ceil_atoms": {
            policy.basis.value: fee_quote(
                laundering_payoffs, laundering_prices, 100, policy
            ).terminal_ceil_atoms
            for policy in policies
        },
    }
    report = {
        "admission_quote": admission.__dict__,
        "fee_rows": fee_rows,
        "zero_price_laundering": zero_price_laundering,
        "feed": {
            "capital_shares": list(admitted.feed.capital_shares),
            "reserve_balance": admitted.feed.reserve_balance,
            "reserve_cap": admitted.feed.reserve_cap,
        },
        "status": "MODEL_ONLY_PROPOSED_POLICY",
    }
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
