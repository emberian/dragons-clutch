#!/usr/bin/env python3
"""Emit a stable adversarial comparison report for ScoreV1 and ScoreV2-Q."""

from __future__ import annotations

import json

from model import (
    ExecutedLeg,
    RiskObjectiveV2,
    SelectionKeyV2,
    Side,
    owner_normalized_direct_flow,
    price_weighted_gini_numerator,
    score_v1_primary,
)


def render(flow: tuple[int, ...], prices: tuple[int, ...]) -> dict[str, object]:
    return {
        "direct_flow": list(flow),
        "prices": list(prices),
        "score_v1_primary": score_v1_primary(flow, prices, 10_000),
        "score_v2_certified_risk_flow_atoms": (
            RiskObjectiveV2.from_direct_flow(flow).certified_risk_flow_atoms
        ),
        "observational_gini_numerator": price_weighted_gini_numerator(
            flow, prices, 10_000
        ),
    }


def main() -> None:
    same_key = owner_normalized_direct_flow(
        2,
        (
            ExecutedLeg("actor", 0, Side.BUY, 7),
            ExecutedLeg("actor", 0, Side.SELL, 7),
        ),
    )
    split_keys = owner_normalized_direct_flow(
        2,
        (
            ExecutedLeg("actor-a", 0, Side.BUY, 7),
            ExecutedLeg("actor-b", 0, Side.SELL, 7),
        ),
    )
    empty = SelectionKeyV2.from_candidate((0, 0), 0, 0, bytes([255]) * 32)
    complete_set = SelectionKeyV2.from_candidate((7, 7), 0, 0, bytes(32))
    report = {
        "identity_impossibility": (
            "honest and common-control worlds with the same public-key transcript "
            "are observationally identical"
        ),
        "price_quality": "not part of ScoreV2-Q",
        "scenarios": {
            "complete_set_wash_midpoint": render((7, 7), (5_000, 5_000)),
            "midpoint_tail_claim": render((7, 0), (5_000, 5_000)),
            "one_percent_tail_claim": render((7, 0), (100, 9_900)),
            "zero_price_tail_claim": render((7, 0), (0, 10_000)),
        },
        "selection": {
            "empty_beats_complete_set_wash": empty.is_better_than(complete_set),
            "economic_objectives_equal": empty.objective == complete_set.objective,
        },
        "v1_owner_normalization_counterexample": {
            "same_key_direct_flow": list(same_key),
            "split_keys_direct_flow": list(split_keys),
        },
    }
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
